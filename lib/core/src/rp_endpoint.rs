//! Model for endpoints

use {Attributes, Loc, RpChannel, RpPathSpec};
use std::default;
use std::rc::Rc;

#[derive(Debug, Clone, Serialize)]
pub enum RpHttpMethod {
    GET,
    POST,
    PUT,
    UPDATE,
    DELETE,
    PATCH,
    HEAD,
}

impl RpHttpMethod {
    /// Treat this method to an all uppercase string representing the method.
    pub fn as_str(&self) -> &str {
        use self::RpHttpMethod::*;

        match *self {
            GET => "GET",
            POST => "POST",
            PUT => "PUT",
            UPDATE => "UPDATE",
            DELETE => "DELETE",
            PATCH => "PATCH",
            HEAD => "HEAD",
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum RpAccept {
    #[serde(rename = "json")]
    Json,
    #[serde(rename = "text")]
    Text,
}

impl default::Default for RpAccept {
    fn default() -> Self {
        RpAccept::Json
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct RpEndpointHttp {
    /// Path specification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<RpPathSpec>,
    /// Argument that is the body of the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<Rc<RpEndpointArgument>>,
    /// HTTP method.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<RpHttpMethod>,
    /// Accepted media types.
    pub accept: RpAccept,
}

/// An argument to an endpont.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RpEndpointArgument {
    /// Identifier of the argument.
    pub ident: Loc<String>,
    /// Safe identifier for the argument.
    pub safe_ident: Option<String>,
    /// Channel of the argument.
    pub channel: Loc<RpChannel>,
}

impl RpEndpointArgument {
    /// Access the actual identifier of the endpoint argument.
    pub fn ident(&self) -> &str {
        self.ident.as_str()
    }

    /// Access the safe identifier for the endpoint argument.
    pub fn safe_ident(&self) -> &str {
        self.safe_ident
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or_else(|| self.ident.as_str())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RpEndpoint {
    /// Name of the endpoint. Guaranteed to be unique.
    pub ident: String,
    /// Safe identifier of the endpoint, avoiding any language-specific keywords.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safe_ident: Option<String>,
    /// Name of the endpoint. This is the name which is being sent over the wire.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Comments for documentation.
    pub comment: Vec<String>,
    /// Attributes associated with the endpoint.
    pub attributes: Attributes,
    /// Request type that this endpoint expects.
    pub arguments: Vec<Rc<RpEndpointArgument>>,
    /// Response type that this endpoint responds with.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<Loc<RpChannel>>,
    /// HTTP configuration.
    pub http: RpEndpointHttp,
}

impl RpEndpoint {
    pub fn id_parts<F>(&self, filter: F) -> Vec<String>
    where
        F: Fn(&str) -> String,
    {
        vec![filter(self.ident.as_str())]
    }

    /// Get the name of the endpoint.
    pub fn name(&self) -> &str {
        self.name
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or(self.ident())
    }

    /// Safe identifier of the endpoint.
    pub fn safe_ident(&self) -> &str {
        self.safe_ident
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or(self.ident.as_str())
    }

    /// Get the identifier of the endpoint.
    pub fn ident(&self) -> &str {
        self.ident.as_str()
    }

    /// If endpoint has metadata for HTTP.
    pub fn has_http_support(&self) -> bool {
        if !self.http.path.is_some() {
            return false;
        }

        true
    }
}
