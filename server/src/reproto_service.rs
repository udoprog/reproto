use errors::{Error, Result};
use flate2::FlateReadExt;
use futures::future::{ok, Future};
use futures_cpupool::CpuPool;
use hyper::header::{ContentEncoding, ContentLength, ContentType, Encoding, Headers};
use hyper::mime;
use hyper::server::{Request, Response, Service};
use hyper::{self, Method, StatusCode};
use io;
use reproto_repository::{to_checksum, Checksum, FileObjects, Objects};
use std::fs::File;
use std::io::Read;
use std::io::{Seek, SeekFrom};
use std::sync::{Arc, Mutex};
use tempfile;

const CHECKSUM_MISMATCH: &'static str = "checksum mismatch";
const BAD_OBJECT_ID: &'static str = "bad object id";

/// ## Read the contents of the file into a byte-vector
fn read_contents<'a, R: AsMut<Read + 'a>>(mut reader: R) -> Result<Vec<u8>> {
    let mut content = Vec::new();
    reader.as_mut().read_to_end(&mut content)?;
    Ok(content)
}

pub struct ReprotoService {
    pub max_file_size: u64,
    pub pool: Arc<CpuPool>,
    pub objects: Arc<Mutex<FileObjects>>,
}

type EncodingFn = fn(&File) -> Result<Box<Read>>;

impl ReprotoService {
    fn no_encoding(input: &File) -> Result<Box<Read>> {
        Ok(Box::new(input.try_clone()?))
    }

    fn gzip_encoding(input: &File) -> Result<Box<Read>> {
        Ok(Box::new(input.try_clone()?.gz_decode()?))
    }

    fn pick_encoding(headers: &Headers) -> fn(&File) -> Result<Box<Read>> {
        if let Some(h) = headers.get::<ContentEncoding>() {
            // client encoded as gzip
            if h.0
                .iter()
                .find(|encoding| **encoding == Encoding::Gzip)
                .is_some()
            {
                return Self::gzip_encoding;
            }
        }

        Self::no_encoding
    }

    fn not_found() -> Response {
        Response::new().with_status(StatusCode::NotFound)
    }

    fn get_index(&self) -> Result<Box<Future<Item = Response, Error = Error>>> {
        let m = "<html></html>";

        let res = ok(Response::new()
            .with_status(StatusCode::Ok)
            .with_header(ContentLength(m.len() as u64))
            .with_body(m));

        Ok(Box::new(res))
    }

    fn get_objects(&self, id: &str) -> Result<Box<Future<Item = Response, Error = Error>>> {
        let objects = self.objects.clone();

        let checksum = Checksum::from_str(id).map_err(|_| Error::BadRequest(BAD_OBJECT_ID.into()))?;

        // No async I/O, use pool
        Ok(Box::new(self.pool.spawn_fn(move || {
            let result = objects
                .lock()
                .map_err(|_| "lock poisoned")?
                .get_object(&checksum)?;

            let object = match result {
                Some(object) => object,
                None => return Ok(Self::not_found()),
            };

            let bytes = read_contents(object.read()?)?;

            Ok(Response::new()
                .with_status(StatusCode::Ok)
                .with_header(ContentLength(bytes.len() as u64))
                .with_body(bytes))
        })))
    }

    /// Put the uploaded object into the object repository.
    fn put_uploaded_object<F>(
        &self,
        body: F,
        checksum: Checksum,
        encoding: EncodingFn,
    ) -> Box<Future<Item = Response, Error = Error>>
    where
        F: 'static + Future<Item = File, Error = Error>,
    {
        let pool = self.pool.clone();
        let objects = self.objects.clone();

        let upload = body.and_then(move |mut tmp| {
            pool.spawn_fn(move || {
                tmp.seek(SeekFrom::Start(0))?;
                let mut checksum_read = encoding(&tmp)?;

                let actual = to_checksum(&mut checksum_read)?;

                if actual != checksum {
                    info!("{} != {}", actual, checksum);

                    return Ok(Response::new()
                        .with_body(CHECKSUM_MISMATCH)
                        .with_status(StatusCode::BadRequest));
                }

                info!("Uploading object: {}", checksum);

                tmp.seek(SeekFrom::Start(0))?;
                let mut read = encoding(&tmp)?;

                objects.lock().map_err(|_| "lock poisoned")?.put_object(
                    &checksum,
                    &mut read,
                    false,
                )?;

                Ok(Response::new().with_status(StatusCode::Ok))
            })
        });

        Box::new(upload)
    }

    fn put_objects(
        &self,
        id: &str,
        req: Request,
    ) -> Result<Box<Future<Item = Response, Error = Error>>> {
        let checksum = Checksum::from_str(id).map_err(|_| Error::BadRequest(BAD_OBJECT_ID.into()))?;

        if let Some(len) = req.headers().get::<ContentLength>() {
            if len.0 > self.max_file_size {
                return Err(Error::BadRequest("file too large".into()).into());
            }
        } else {
            return Err(Error::BadRequest("missing content-length".into()).into());
        }

        let encoding = Self::pick_encoding(req.headers());

        info!("Creating temporary file");
        let body = io::stream_to_file(tempfile::tempfile()?, self.pool.clone(), req.body())
            .map_err(|e| format!("Failed to stream file: {}", e.display()).into());
        Ok(self.put_uploaded_object(body, checksum, encoding))
    }

    fn inner_call<'a, I>(
        &self,
        req: Request,
        path: I,
    ) -> Result<Box<Future<Item = Response, Error = Error>>>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut it = path.into_iter();

        let a = it.next();
        let b = it.next();
        let c = it.next();

        match (req.method(), a, b, c) {
            (&Method::Get, Some(""), None, None) => {
                return self.get_index();
            }
            (&Method::Get, Some("objects"), Some(id), None) => {
                return self.get_objects(id);
            }
            (&Method::Put, Some("objects"), Some(id), None) => {
                return self.put_objects(id, req);
            }
            _ => {}
        }

        Ok(Box::new(ok(Self::not_found())))
    }

    fn handle_error(e: Error) -> Response {
        match e {
            Error::BadRequest(message) => {
                return Response::new()
                    .with_status(StatusCode::BadRequest)
                    .with_header(ContentLength(message.len() as u64))
                    .with_header(ContentType(mime::TEXT_PLAIN))
                    .with_body(message)
            }
            Error::Other(error) => {
                error!("{}", error.message());

                for e in error.causes().skip(1) {
                    error!("caused by: {}", e.message());
                }

                if let Some(backtrace) = error.backtrace() {
                    error!("{:?}", backtrace);
                }

                return Response::new().with_status(StatusCode::InternalServerError);
            }
        }
    }
}

impl Service for ReprotoService {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = Box<Future<Item = Response, Error = hyper::Error>>;

    fn call(&self, req: Request) -> Self::Future {
        let full_path = String::from(req.path());

        let path = full_path.split('/').skip(1);

        Box::new(
            self.inner_call(req, path)
                .unwrap_or_else(|e| Box::new(ok(Self::handle_error(e))))
                .or_else(|e| ok(Self::handle_error(e))),
        )
    }
}
