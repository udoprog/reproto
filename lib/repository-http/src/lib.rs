//! ## Load objects from a remote repository over HTTP

extern crate futures;
extern crate hyper;
extern crate hyper_rustls;
extern crate reproto_core as core;
extern crate reproto_repository as repository;
extern crate tokio_core;
extern crate url;

use core::errors::{Error, Result};
use core::{BytesObject, Object};
use futures::future::{err, ok};
use futures::{Future, Stream};
use hyper::header::ContentLength;
use hyper::{Client, Method, Request, StatusCode};
use repository::{CachedObjects, Checksum, HexSlice, Objects, ObjectsConfig};
use std::io::Read;
use std::sync::Arc;
use std::time::Duration;
use tokio_core::reactor::Core;
use url::Url;

pub struct HttpObjects {
    url: Url,
    core: Core,
}

impl HttpObjects {
    pub fn new(url: Url, core: Core) -> HttpObjects {
        HttpObjects {
            url: url,
            core: core,
        }
    }

    fn checksum_url(&self, checksum: &Checksum) -> Result<hyper::Uri> {
        let url = self.url.join(HexSlice::new(checksum).to_string().as_ref())?;

        let url = url.to_string()
            .parse::<hyper::Uri>()
            .map_err(|e| format!("Failed to parse URL: {}: {}", e, url))?;

        Ok(url)
    }

    fn handle_request(
        &mut self,
        request: Request,
    ) -> Box<Future<Item = (Vec<u8>, StatusCode), Error = Error>> {
        let handle = self.core.handle();

        let client = Client::configure()
            .connector(hyper_rustls::HttpsConnector::new(4, &handle))
            .build(&handle);

        let body_and_status = client
            .request(request)
            .map_err::<_, Error>(|e| format!("Request to repository failed: {}", e).into())
            .and_then(|res| {
                let status = res.status().clone();

                res.body()
                    .map_err::<Error, _>(|e| format!("Failed to perform request: {}", e).into())
                    .fold(Vec::new(), |mut out: Vec<u8>, chunk| {
                        out.extend(chunk.as_ref());
                        ok::<_, Error>(out)
                    })
                    .map(move |body| (body, status))
            });

        Box::new(body_and_status)
    }
}

impl Objects for HttpObjects {
    fn put_object(&mut self, checksum: &Checksum, source: &mut Read, _force: bool) -> Result<()> {
        let mut buffer = Vec::new();
        source.read_to_end(&mut buffer)?;

        let url = self.checksum_url(checksum)?;

        let mut request = Request::new(Method::Put, url);
        request
            .headers_mut()
            .set(ContentLength(buffer.len() as u64));
        request.set_body(buffer);

        let work = self.handle_request(request).and_then(|(body, status)| {
            if !status.is_success() {
                if let Ok(body) = String::from_utf8(body) {
                    return err(format!("bad response: {}: {}", status, body).into());
                }

                return err(format!("bad response: {}", status).into());
            }

            ok(())
        });

        self.core.run(work)?;
        Ok(())
    }

    fn get_object(&mut self, checksum: &Checksum) -> Result<Option<Box<Object>>> {
        let url = self.checksum_url(checksum)?;
        let name = url.to_string();

        let request = Request::new(Method::Get, url);

        let work = self.handle_request(request).and_then(|(body, status)| {
            if status.is_success() {
                return ok(Some(body));
            }

            if status == StatusCode::NotFound {
                return ok(None);
            }

            if let Ok(body) = String::from_utf8(body) {
                return err(format!("bad response: {}: {}", status, body).into());
            }

            return err(format!("bad response: {}", status).into());
        });

        let out = self.core.run(work)?;
        let out = out.map(Arc::new);
        Ok(out.map(|out| Box::new(BytesObject::new(name, out)) as Box<Object>))
    }
}

/// Load objects from an HTTP url.
pub fn objects_from_url(config: ObjectsConfig, url: &Url) -> Result<Box<Objects>> {
    let core = Core::new()?;

    let http_objects = HttpObjects::new(url.clone(), core);

    if let Some(cache_dir) = config.cache_dir {
        let missing_cache_time = config
            .missing_cache_time
            .unwrap_or_else(|| Duration::new(60, 0));

        return Ok(Box::new(CachedObjects::new(
            cache_dir,
            missing_cache_time,
            http_objects,
        )));
    }

    Ok(Box::new(http_objects))
}
