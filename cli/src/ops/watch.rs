//! build command

use clap::{App, Arg, ArgMatches, SubCommand};
use core::Context;
use core::errors::Result;
use output::Output;
use std::rc::Rc;

pub fn options<'a, 'b>() -> App<'a, 'b> {
    let out = SubCommand::with_name("watch")
        .about("Setup a file watcher that automatically builds specifications");

    let out = out.arg(
        Arg::with_name("lang")
            .long("lang")
            .takes_value(true)
            .help("Language to build for"),
    );

    let out = out.arg(
        Arg::with_name("delete")
            .long("delete")
            .help("Delete files that are no longer part of the build"),
    );

    out
}

#[cfg(not(feature = "notify"))]
pub fn entry(_: Rc<Context>, _: &ArgMatches, _: &Output) -> Result<()> {
    Err("`watch` command is not supported: `notify` feature is disabled".into())
}

#[cfg(feature = "notify")]
pub fn entry(ctx: Rc<Context>, matches: &ArgMatches, output: &Output) -> Result<()> {
    use build_spec::{convert_lang, environment_with_hook, manifest, manifest_preamble};
    use manifest::Language;
    use notify::{RecommendedWatcher, RecursiveMode, Watcher};
    use std::cell::RefCell;
    use std::collections::HashSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    // files discovered by the environment
    let paths: Rc<RefCell<HashSet<PathBuf>>> = Rc::new(RefCell::new(HashSet::new()));
    // files to watch
    let mut watching: HashSet<PathBuf> = HashSet::new();

    let (tx, rx) = mpsc::channel();
    let mut watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_secs(0))?;

    let written_files: Rc<RefCell<HashSet<PathBuf>>> = Rc::new(RefCell::new(HashSet::new()));
    let written_dirs: Rc<RefCell<HashSet<PathBuf>>> = Rc::new(RefCell::new(HashSet::new()));

    // current set of known files
    let mut files = HashSet::new();

    let delete = matches.is_present("delete");

    loop {
        info!("updating project");

        let update = match try_compile(ctx.clone(), matches, &paths, &written_files, &written_dirs)
        {
            Err(e) => {
                let ctx_errors = ctx.errors()?;

                if ctx_errors.is_empty() {
                    output.handle_error(&e)?;
                } else {
                    output.handle_context(ctx_errors.as_ref())?;
                }

                false
            }
            Ok(_) => true,
        };

        // if last build _was_ successful, delete files.
        // Only actually delete things if `--delete` is specified.
        if update {
            let mut written_files = written_files.try_borrow_mut()?;

            let mut removed = Vec::new();

            files.extend(written_files.iter().cloned());

            for p in written_files.symmetric_difference(&files) {
                removed.push(p.to_owned());

                if delete {
                    debug!("deleting: {}", p.display());
                    drain_removed_file(p)?;
                } else {
                    warn!("not deleting: {} (`--delete` is not enabled)", p.display());
                }
            }

            for f in removed {
                files.remove(&f);
            }

            written_files.clear();
            let mut dirs = written_dirs.try_borrow_mut()?;

            if delete {
                drain_created_dirs(&mut dirs)?;
            } else {
                for d in dirs.iter() {
                    warn!("not deleting: {} (`--delete` is not enabled)", d.display());
                }
            }
        }

        // move additional watch paths, unless an error occurred.
        if update {
            let mut paths = paths.try_borrow_mut()?;

            for p in watching.symmetric_difference(&paths) {
                debug!("watch: {}", p.display());

                if let Some(parent) = p.parent() {
                    watcher.watch(parent, RecursiveMode::NonRecursive)?;
                }

                watcher.watch(p, RecursiveMode::NonRecursive)?;
            }

            for p in watching.difference(&paths) {
                debug!("unwatch: {}", p.display());

                if let Some(parent) = p.parent() {
                    watcher.unwatch(parent)?;
                }

                watcher.unwatch(p)?;
            }

            watching.clear();
            watching.extend(paths.drain());
        }

        if watching.is_empty() {
            info!("Nothing being watched, exiting...");
            break;
        }

        let mut sleep = false;

        loop {
            use notify::DebouncedEvent::*;

            match rx.recv() {
                Ok(e) => match e {
                    // special handling to avoid debouncing?
                    NoticeRemove(p) | Remove(p) => {
                        sleep = true;

                        if watching.contains(&p) {
                            break;
                        }
                    }
                    NoticeWrite(p) | Create(p) | Write(p) | Chmod(p) => {
                        if watching.contains(&p) {
                            break;
                        }
                    }
                    Rescan => {
                        break;
                    }
                    Rename(from, to) => {
                        sleep = true;

                        if watching.contains(&from) || watching.contains(&to) {
                            break;
                        }
                    }
                    Error(e, p) => {
                        if let Some(p) = p {
                            return Err(format!("error watching path: {}: {}", p.display(), e).into());
                        }

                        return Err(e.into());
                    }
                },
                Err(e) => return Err(e.into()),
            }
        }

        if sleep {
            // Sleep to permit file changes to settle...
            thread::sleep(Duration::from_secs(1));
        }

        // try to extra eventsto bounce less...
        loop {
            match rx.try_recv() {
                Err(mpsc::TryRecvError::Empty) => break,
                Err(e) => return Err(e.into()),
                Ok(_) => {}
            }
        }
    }

    return Ok(());

    /// Remove a file and all it's parent directories if empty.
    fn drain_removed_file(file: &Path) -> Result<()> {
        if file.is_file() {
            fs::remove_file(&file)?;
        }

        // drain directory, if empty

        let mut parent = file.parent();

        while let Some(p) = parent {
            // will fail unless directory is empty.
            match fs::remove_dir(p) {
                Err(_) => break,
                Ok(_) => {}
            }

            parent = p.parent();
        }

        Ok(())
    }

    /// Remove directories.
    fn drain_created_dirs(dirs: &mut HashSet<PathBuf>) -> Result<()> {
        for d in dirs.drain() {
            debug!("deleting: {}", d.display());
            // we don't care if it succeeds or not.
            let _ = fs::remove_dir(d);
        }

        Ok(())
    }

    fn try_compile(
        ctx: Rc<Context>,
        matches: &ArgMatches,
        paths: &Rc<RefCell<HashSet<PathBuf>>>,
        added_files: &Rc<RefCell<HashSet<PathBuf>>>,
        added_dirs: &Rc<RefCell<HashSet<PathBuf>>>,
    ) -> Result<()> {
        // Access a fresh context.
        let ctx = Rc::new(ctx.as_ref().clone().map_filesystem(|fs| {
            Box::new(stalker::StalkerFilesystem::new(
                fs,
                added_files.clone(),
                added_dirs.clone(),
            ))
        }));

        // TODO: swap out filesystem implementation with a "stalker" implementation that records all
        // files written.

        let preamble = manifest_preamble(matches)?;

        let language = preamble
            .language
            .as_ref()
            .cloned()
            .or_else(|| matches.value_of("lang").and_then(Language::parse))
            .ok_or_else(|| "no language specified either through manifest or cli (--lang)")?;

        let lang = convert_lang(language);

        let manifest = manifest(lang.as_ref(), matches, preamble)?;

        let local_paths = paths.clone();

        let env = environment_with_hook(lang.as_ref(), ctx.clone(), &manifest, move |p| {
            let mut paths = local_paths.try_borrow_mut()?;
            let p = p.to_owned().canonicalize()?;
            paths.insert(p);
            Ok(())
        })?;

        lang.compile(ctx.clone(), env, manifest)?;
        Ok(())
    }
}

#[cfg(feature = "notify")]
mod stalker {
    use core::errors::Result;
    use core::{Filesystem, Handle, RelativePath};
    use std::cell::RefCell;
    use std::collections::HashSet;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;

    /// A filesystem implementation that keeps track of files which have been opened for writing.
    pub struct StalkerFilesystem {
        delegate: Rc<Box<Filesystem>>,
        files: Rc<RefCell<HashSet<PathBuf>>>,
        dirs: Rc<RefCell<HashSet<PathBuf>>>,
    }

    impl StalkerFilesystem {
        pub fn new(
            delegate: Rc<Box<Filesystem>>,
            files: Rc<RefCell<HashSet<PathBuf>>>,
            dirs: Rc<RefCell<HashSet<PathBuf>>>,
        ) -> StalkerFilesystem {
            Self {
                delegate,
                files,
                dirs,
            }
        }
    }

    impl Filesystem for StalkerFilesystem {
        fn open_root(&self, root: Option<&Path>) -> Result<Box<Handle>> {
            let delegate = self.delegate.open_root(root.clone())?;

            return Ok(Box::new(StalkerHandle {
                delegate,
                root: root.clone().map(ToOwned::to_owned),
                files: self.files.clone(),
                dirs: self.dirs.clone(),
            }));
        }
    }

    /// A handle that captures files into a RefCell.
    struct StalkerHandle {
        delegate: Box<Handle>,
        root: Option<PathBuf>,
        files: Rc<RefCell<HashSet<PathBuf>>>,
        dirs: Rc<RefCell<HashSet<PathBuf>>>,
    }

    impl Handle for StalkerHandle {
        fn is_dir(&self, path: &RelativePath) -> bool {
            self.delegate.is_dir(path)
        }

        fn is_file(&self, path: &RelativePath) -> bool {
            self.delegate.is_file(path)
        }

        fn create_dir_all(&self, path: &RelativePath) -> Result<()> {
            if let Some(root) = self.root.as_ref() {
                let mut dirs = self.dirs.try_borrow_mut()?;
                dirs.insert(path.to_path(root));
            }

            self.delegate.create_dir_all(path)
        }

        fn create(&self, path: &RelativePath) -> Result<Box<io::Write>> {
            match self.delegate.create(path) {
                Ok(w) => {
                    if let Some(root) = self.root.as_ref() {
                        let mut files = self.files.try_borrow_mut()?;
                        files.insert(path.to_path(root));
                    }

                    Ok(w)
                }
                r => r,
            }
        }
    }
}
