//! build command
use clap::{App, Arg, SubCommand};

#[cfg(feature = "self-updates")]
mod internal {
    extern crate flate2;
    extern crate futures;
    extern crate hyper;
    extern crate hyper_rustls;
    extern crate same_file;
    extern crate tar;
    extern crate tokio_core;

    use self::flate2::FlateReadExt;
    use self::futures::future::{err, ok};
    use self::futures::{Future, Stream};
    use self::hyper::header::Location;
    use self::hyper::{Client, Method, Request, Response, StatusCode, Uri};
    use self::tar::Archive;
    use self::tokio_core::reactor::{Core, Handle};
    use VERSION;
    use clap::ArgMatches;
    use core::errors::{Error, Result};
    use core::{Context, Version};
    use env;
    use std::fs::{self, File};
    use std::io::{self, Cursor};
    use std::path::Path;
    use std::rc::Rc;
    use std::sync::Arc;
    use url::Url;

    #[cfg(target_os = "macos")]
    mod os {
        pub const PLATFORM: Option<&str> = Some("osx");
        pub const EXT: Option<&str> = Some("");
    }

    #[cfg(target_os = "linux")]
    mod os {
        pub const PLATFORM: Option<&str> = Some("linux");
        pub const EXT: Option<&str> = Some("");
    }

    #[cfg(target_os = "windows")]
    mod os {
        pub const PLATFORM: Option<&str> = Some("windows");
        pub const EXT: Option<&str> = Some(".exe");
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    mod os {
        pub const PLATFORM: Option<&str> = None;
        pub const EXT: Option<&str> = None;
    }

    #[cfg(target_arch = "x86_64")]
    const ARCH: Option<&str> = Some("x86_64");

    #[cfg(not(target_arch = "x86_64"))]
    const ARCH: Option<&str> = None;

    use self::os::EXT;
    use self::os::PLATFORM;

    const DEFAULT_URL: &str = "https://storage.googleapis.com/reproto-releases/";

    pub fn entry(_: Rc<Context>, m: &ArgMatches) -> Result<()> {
        let config = env::ConfigEnv::new()?
            .ok_or_else(|| format!("could not setup the reproto environment"))?;

        check_path(&config)?;

        let force = m.is_present("force");
        let prerelease = m.is_present("prerelease");

        let arch = ARCH.clone().or(m.value_of("arch")).ok_or_else(|| {
            format!("Architecture could not be detected, and is not specified with `--arch`")
        })?;

        let platform = PLATFORM.clone().or(m.value_of("platform")).ok_or_else(|| {
            format!("Platform could not be detected, and is not specified with `--platform`")
        })?;

        let ext = EXT.clone().or(m.value_of("ext")).ok_or_else(|| {
            format!(
                "Binary could not be detected, and is not specified with `--ext`. Should be \
                 something like `reproto` or `reproto.exe`"
            )
        })?;

        let current = Version::parse(VERSION)?;

        let url = m.value_of("url").unwrap_or(DEFAULT_URL);
        let url = Url::parse(url)?;

        let mut core = Core::new()?;
        let handle = core.handle();

        let mut client = UpdateClient::new(handle, url)?;

        let mut releases = core.run(client.get_releases())?;

        releases.sort();

        let releases = match prerelease {
            true => releases,
            false => releases
                .into_iter()
                .filter(|r| !r.is_prerelease())
                .collect(),
        };

        let candidate = if !force {
            releases.into_iter().filter(|r| r > &current).last()
        } else {
            releases.into_iter().last()
        };

        let version = match candidate {
            Some(version) => version,
            None => {
                info!("reproto is up-to-date!");
                return Ok(());
            }
        };

        let tuple = format!("{}-{}-{}", version, platform, arch);

        let archived = config
            .releases_dir
            .join(format!("reproto-{}{}", tuple, ext));
        let binary = format!("reproto{}", ext);

        if !archived.is_file() || force {
            let release = core.run(client.get_release(&version, platform, arch))
                .map_err(|e| format!("{}: failed to download archive: {}", tuple, e.display()))?;

            download_archive(release, &archived, &binary).map_err(|e| {
                format!(
                    "failed to download archive to: {}: {}",
                    archived.display(),
                    e.display()
                )
            })?;

            info!("wrote: {}", archived.display());
        }

        let bin = config.bin_home.join(binary);

        setup_symlink(&archived, &bin).map_err(|e| {
            format!(
                "failed to setup symlink to: {}: {}",
                bin.display(),
                e.display()
            )
        })?;

        return Ok(());

        /// Checks that bin_home is in PATH, or warns otherwise.
        fn check_path(config: &env::ConfigEnv) -> Result<()> {
            let mut bin_in_path = false;

            if config.bin_home.is_dir() {
                if let Some(paths) = ::std::env::var_os("PATH") {
                    for path in ::std::env::split_paths(&paths) {
                        if !path.is_dir() {
                            continue;
                        }

                        if same_file::is_same_file(path, &config.bin_home)? {
                            bin_in_path = true;
                        }
                    }
                }
            }

            if !bin_in_path {
                warn!(
                    "{}: is not in your PATH. This is required for reproto to work!",
                    config.bin_home.display()
                );
            }

            Ok(())
        }

        fn download_archive(release: Vec<u8>, out: &Path, binary: &str) -> Result<()> {
            let mut archive = Archive::new(Cursor::new(release).gz_decode()?);

            for file in archive.entries()? {
                let mut file = file?;

                {
                    let path_bytes = file.header().path_bytes();
                    let path = ::std::str::from_utf8(path_bytes.as_ref())?;

                    if path != binary {
                        warn!("got unexpected file in archive: {}", path);
                        continue;
                    }
                }

                if let Some(parent) = out.parent() {
                    if !parent.is_dir() {
                        info!("creating directory: {}", parent.display());
                        fs::create_dir_all(parent)?;
                    }
                }

                info!("writing: {}", out.display());
                let mut w = File::create(out)?;
                io::copy(&mut file, &mut w)?;

                let metadata = w.metadata()?;
                let mut perm = metadata.permissions();
                set_executable(&mut perm);
                w.set_permissions(perm)?;
                break;
            }

            return Ok(());

            #[cfg(any(target_os = "linux", target_os = "macos"))]
            fn set_executable(p: &mut fs::Permissions) {
                use std::os::unix::fs::PermissionsExt;
                p.set_mode(0755);
            }

            #[cfg(target_os = "windows")]
            fn set_executable(_p: &mut fs::Permissions) {
                // nothing to do on windows.
            }

            #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
            fn set_executable(_p: &mut fs::Permissions) {
                warn!("cannot update permissions on this platform");
            }
        }

        fn setup_symlink(archived: &Path, bin: &Path) -> Result<()> {
            if let Some(parent) = bin.parent() {
                if !parent.is_dir() {
                    info!("creating directory: {}", parent.display());
                    fs::create_dir_all(parent)?;
                }
            }

            info!("creating symlink: {}", bin.display());

            if bin.is_file() {
                fs::remove_file(bin)?;
            }

            symlink(archived, bin)?;
            return Ok(());

            #[cfg(not(target_os = "windows"))]
            fn symlink(archived: &Path, bin: &Path) -> Result<()> {
                use std::os::unix;
                unix::fs::symlink(archived, bin)?;
                Ok(())
            }

            #[cfg(target_os = "windows")]
            fn symlink(archived: &Path, bin: &Path) -> Result<()> {
                use std::os::windows;
                windows::fs::symlink_file(archived, bin)?;
                Ok(())
            }
        }
    }

    struct UpdateClient {
        client: Arc<Client<hyper_rustls::HttpsConnector>>,
        url: Url,
    }

    impl UpdateClient {
        pub fn new(handle: Handle, url: Url) -> Result<Self> {
            let client = Client::configure()
                .connector(hyper_rustls::HttpsConnector::new(4, &handle))
                .build(&handle);

            Ok(Self {
                client: Arc::new(client),
                url,
            })
        }

        /// Handle redirects for request.
        fn handle_redirect(
            client: Arc<Client<hyper_rustls::HttpsConnector>>,
            res: &Response,
        ) -> Option<Box<Future<Item = Vec<u8>, Error = Error>>> {
            let should_redirect = match res.status() {
                StatusCode::MovedPermanently
                | StatusCode::Found
                | StatusCode::SeeOther
                | StatusCode::TemporaryRedirect
                | StatusCode::PermanentRedirect => true,
                _ => false,
            };

            if should_redirect {
                let uri = if let Some(loc) = res.headers().get::<Location>() {
                    match ::std::str::from_utf8(loc.as_bytes())
                        .map_err(Error::from)
                        .and_then(|s| s.parse::<Uri>().map_err(Error::from))
                    {
                        Ok(uri) => uri,
                        Err(e) => return Some(Box::new(err(e))),
                    }
                } else {
                    return None;
                };

                let req = Request::new(Method::Get, uri);
                return Some(Box::new(Self::request(client, req)));
            }

            return None;
        }

        /// Perform the given request.
        fn request(
            client: Arc<Client<hyper_rustls::HttpsConnector>>,
            req: Request,
        ) -> Box<Future<Item = Vec<u8>, Error = Error>> {
            let inner_client = Arc::clone(&client);

            Box::new(
                client
                    .request(req)
                    .map_err(|e| Error::from(e))
                    .and_then(move |res| {
                        if let Some(future) = Self::handle_redirect(inner_client, &res) {
                            return future;
                        }

                        let status = res.status().clone();

                        let fut = res.body()
                            .map_err(|e| Error::from(e))
                            .fold(Vec::new(), |mut out: Vec<u8>, chunk| {
                                out.extend(chunk.as_ref());
                                ok::<_, Error>(out)
                            })
                            .map(move |body| (body, status))
                            .and_then(|(body, status)| {
                                if !status.is_success() {
                                    if let Ok(body) = String::from_utf8(body) {
                                        return err(format!("bad response: {}: {}", status, body).into());
                                    }

                                    return err(format!("bad response: {}", status).into());
                                }

                                ok(body)
                            });

                        Box::new(fut)
                    }),
            )
        }

        pub fn get_releases(&mut self) -> Box<Future<Item = Vec<Version>, Error = Error>> {
            let url = match self.url.join("releases") {
                Err(e) => return Box::new(err(e.into())),
                Ok(url) => url,
            };

            let uri = match url.as_ref().parse::<Uri>() {
                Err(e) => return Box::new(err(e.into())),
                Ok(uri) => uri,
            };

            let request = Request::new(Method::Get, uri);

            let url = url.clone();

            let future = Self::request(Arc::clone(&self.client), request)
                .and_then(|body| {
                    let body = match String::from_utf8(body) {
                        Err(e) => return err(format!("body is not utf-8: {}", e).into()),
                        Ok(body) => body,
                    };

                    let mut out = Vec::new();

                    for line in body.split('\n') {
                        let line = line.trim();

                        if line.is_empty() {
                            continue;
                        }

                        let version = match Version::parse(line) {
                            Err(e) => return err(e.into()),
                            Ok(version) => version,
                        };

                        out.push(version);
                    }

                    ok(out)
                })
                .map_err(move |e| format!("request to `{}` failed: {}", url, e.display()).into());

            Box::new(future)
        }

        pub fn get_release(
            &mut self,
            version: &Version,
            platform: &str,
            arch: &str,
        ) -> Box<Future<Item = Vec<u8>, Error = Error>> {
            let url = match self.url
                .join(&format!("reproto-{}-{}-{}.tar.gz", version, platform, arch))
            {
                Err(e) => return Box::new(err(e.into())),
                Ok(url) => url,
            };

            let uri = match url.as_ref().parse::<Uri>() {
                Err(e) => return Box::new(err(e.into())),
                Ok(uri) => uri,
            };

            let request = Request::new(Method::Get, uri);

            let url = url.clone();

            let future = Self::request(Arc::clone(&self.client), request)
                .map_err(move |e| format!("request to `{}` failed: {}", url, e.display()).into());

            Box::new(future)
        }
    }
}

#[cfg(not(feature = "self-updates"))]
mod internal {
    use clap::ArgMatches;
    use core::Context;
    use core::errors::Result;
    use std::rc::Rc;

    pub fn entry(_: Rc<Context>, _: &ArgMatches) -> Result<()> {
        return Err("support for self-updates is not enabled".into());
    }
}

pub use self::internal::entry;

pub fn options<'a, 'b>() -> App<'a, 'b> {
    let out = SubCommand::with_name("self-update").about("Update reproto");

    let out = out.arg(
        Arg::with_name("url")
            .long("url")
            .takes_value(true)
            .help("URL to download updates from."),
    );

    let out = out.arg(
        Arg::with_name("arch")
            .long("arch")
            .takes_value(true)
            .help("Architecture to update for"),
    );

    let out = out.arg(
        Arg::with_name("platform")
            .long("platform")
            .takes_value(true)
            .help("Architecture to update for"),
    );

    let out =
        out.arg(Arg::with_name("ext").long("ext").takes_value(true).help(
            "File extension of binary to expect, should be something like `.exe` on windows.",
        ));

    let out = out.arg(
        Arg::with_name("force")
            .short("f")
            .long("force")
            .help("Force downloading the latest release, even though it is already installed"),
    );

    let out = out.arg(
        Arg::with_name("prerelease")
            .long("pre")
            .help("Download a pre-release if available"),
    );

    out
}
