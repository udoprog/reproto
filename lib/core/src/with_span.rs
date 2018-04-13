//! Report errors into the context.
use errors::Error;
use {Diagnostics, Span};

pub trait WithSpan<T> {
    /// Report the span to the diagnostics and convert the result into a unit error if failed.
    fn with_span<S: Into<Span>>(self, diag: &mut Diagnostics, span: S) -> Result<T, ()>;
}

impl<T> WithSpan<T> for Result<T, Error> {
    fn with_span<S: Into<Span>>(self, diag: &mut Diagnostics, span: S) -> Result<T, ()> {
        match self {
            Ok(value) => Ok(value),
            Err(e) => {
                diag.err(span, e.display());
                Err(())
            }
        }
    }
}
