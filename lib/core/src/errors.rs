use error_pos::ErrorPos;
use std::borrow::Cow;
use std::env;
use std::ffi;
use std::fmt;
use std::result;
use std::sync::atomic;
use with_pos::WithPos;

const RUST_BACKTRACE: &str = "RUST_BACKTRACE";

#[cfg(all(feature = "backtrace", feature = "std"))]
mod internal {
    pub const HAS_BACKTRACE: bool = true;

    pub use backtrace::Backtrace;
}

#[cfg(not(all(feature = "backtrace", feature = "std")))]
mod internal {
    pub const HAS_BACKTRACE: bool = false;

    use std::fmt;

    /// Fake internal representation.
    pub struct Backtrace(());

    impl Backtrace {
        pub fn new() -> Backtrace {
            Backtrace(())
        }
    }

    impl fmt::Debug for Backtrace {
        fn fmt(&self, _fmt: &mut fmt::Formatter) -> fmt::Result {
            Ok(())
        }
    }
}

pub use self::internal::{Backtrace, HAS_BACKTRACE};

pub type Result<T> = result::Result<T, Error>;

/// Extra convenience functions for results based on core errors.
pub trait ResultExt<T>
where
    Self: Sized,
{
    fn chain_err<C, D>(self, chain: C) -> Result<T>
    where
        C: FnOnce() -> D,
        D: Into<Error>;
}

impl<T> ResultExt<T> for result::Result<T, Error> {
    fn chain_err<C, D>(self, chain: C) -> Result<T>
    where
        C: FnOnce() -> D,
        D: Into<Error>,
    {
        match self {
            Err(e) => {
                let mut new = chain().into();
                new.cause = Some(Box::new(e));
                Err(new)
            }
            Ok(value) => Ok(value),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Causes<'a> {
    current: Option<&'a Error>,
}

impl<'a> Iterator for Causes<'a> {
    type Item = &'a Error;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(e) = self.current {
            self.current = e.cause();
            return Some(e);
        }

        None
    }
}

/// Error display type.
///
/// Since Error can't implement fmt::Display, this acts as a proxy.
pub struct Display {
    message: Cow<'static, str>,
}

impl fmt::Display for Display {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.write_str(self.message.as_ref())
    }
}

fn is_backtrace_enabled<F: Fn(&str) -> Option<ffi::OsString>>(get_var: F) -> bool {
    match get_var(RUST_BACKTRACE) {
        Some(ref val) if val != "0" => true,
        _ => false,
    }
}

pub struct Error {
    message: Cow<'static, str>,
    pos: Option<ErrorPos>,
    cause: Option<Box<Error>>,
    suppressed: Vec<Error>,
    backtrace: Option<Backtrace>,
}

impl Error {
    pub fn new<M: Into<Cow<'static, str>>>(message: M) -> Self {
        Self {
            message: message.into(),
            pos: None,
            cause: None,
            suppressed: Vec::new(),
            backtrace: Self::new_backtrace(),
        }
    }

    fn new_backtrace() -> Option<Backtrace> {
        if !HAS_BACKTRACE {
            return None;
        }

        static ENABLED: atomic::AtomicUsize = atomic::ATOMIC_USIZE_INIT;

        match ENABLED.load(atomic::Ordering::SeqCst) {
            0 => {
                let enabled = is_backtrace_enabled(|var| env::var_os(var));

                ENABLED.store(enabled as usize + 1, atomic::Ordering::SeqCst);

                if !enabled {
                    return None;
                }
            }
            1 => return None,
            _ => {}
        }

        return Some(Backtrace::new());
    }

    /// Set the position for this error.
    pub fn with_pos<P: Into<ErrorPos>>(self, pos: P) -> Error {
        Error {
            pos: Some(pos.into()),
            ..self
        }
    }

    /// Set the position for this error.
    pub fn with_suppressed<S: IntoIterator<Item = Error>>(self, suppressed: S) -> Error {
        Error {
            suppressed: suppressed.into_iter().collect(),
            ..self
        }
    }

    /// Convert errro into a type that is `fmt::Display`.
    ///
    /// WARNING: drops error information. Only use if absolutely necessary!
    pub fn display(self) -> Display {
        Display { message: self.message }
    }

    /// Get backtrace.
    pub fn backtrace(&self) -> Option<&Backtrace> {
        self.backtrace.as_ref()
    }

    /// Get the message for the error.
    pub fn message(&self) -> &str {
        self.message.as_ref()
    }

    /// Extract the error position, if available.
    pub fn pos(&self) -> Option<&ErrorPos> {
        self.pos.as_ref()
    }

    /// Get the cause of this error.
    pub fn cause(&self) -> Option<&Error> {
        self.cause.as_ref().map(AsRef::as_ref)
    }

    /// Get all suppressed errors.
    pub fn suppressed(&self) -> Vec<&Error> {
        self.suppressed.iter().collect()
    }

    /// Iterate over all causes.
    pub fn causes(&self) -> Causes {
        Causes { current: Some(self) }
    }
}

impl<T> From<T> for Error
where
    T: fmt::Display,
{
    fn from(value: T) -> Error {
        Error::new(value.to_string())
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Error")
            .field("message", &self.message)
            .finish()
    }
}

impl WithPos for Error {
    fn with_pos<E: Into<ErrorPos>>(self, pos: E) -> Self {
        if self.pos.is_some() {
            return self;
        }

        Self {
            pos: Some(pos.into()),
            ..self
        }
    }
}
