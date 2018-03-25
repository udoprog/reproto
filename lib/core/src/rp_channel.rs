//! Data model for request or responses for endpoints

use errors::Result;
use std::fmt;
use {Flavor, Translate, Translator};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum RpChannel<F: 'static>
where
    F: Flavor,
{
    /// Single send.
    Unary { ty: F::Type },
    /// Multiple sends.
    Streaming { ty: F::Type },
}

impl<F: 'static> RpChannel<F>
where
    F: Flavor,
{
    /// Get the type of the channel.
    pub fn ty(&self) -> &F::Type {
        use self::RpChannel::*;

        match *self {
            Unary { ref ty, .. } | Streaming { ref ty, .. } => ty,
        }
    }

    /// Check if channel is streaming.
    pub fn is_streaming(&self) -> bool {
        use self::RpChannel::*;

        match *self {
            Unary { .. } => false,
            Streaming { .. } => true,
        }
    }
}

impl<F: 'static> fmt::Display for RpChannel<F>
where
    F: Flavor,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        if self.is_streaming() {
            write!(fmt, "stream {:?}", self.ty())
        } else {
            write!(fmt, "{:?}", self.ty())
        }
    }
}

impl<F: 'static, T> Translate<T> for RpChannel<F>
where
    F: Flavor,
    T: Translator<Source = F>,
{
    type Source = F;
    type Out = RpChannel<T::Target>;

    /// Translate into different flavor.
    fn translate(self, translator: &T) -> Result<RpChannel<T::Target>> {
        use self::RpChannel::*;

        let out = match self {
            Unary { ty } => Unary {
                ty: translator.translate_type(ty)?,
            },
            Streaming { ty } => Streaming {
                ty: translator.translate_type(ty)?,
            },
        };

        Ok(out)
    }
}
