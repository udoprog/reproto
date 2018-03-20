//! Data model for request or responses for endpoints

use super::RpType;
use std::fmt;

#[derive(Debug, Clone, Serialize)]
pub enum RpChannel {
    /// Single send.
    Unary { ty: RpType },
    /// Multiple sends.
    Streaming { ty: RpType },
}

impl RpChannel {
    /// Get the type of the channel.
    pub fn ty(&self) -> &RpType {
        use self::RpChannel::*;

        match *self {
            Unary { ref ty, .. } |
            Streaming { ref ty, .. } => ty,
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

impl fmt::Display for RpChannel {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        if self.is_streaming() {
            write!(fmt, "stream {}", self.ty())
        } else {
            write!(fmt, "{}", self.ty())
        }
    }
}
