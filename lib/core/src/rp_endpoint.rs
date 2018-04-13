//! Model for endpoints

use errors::Result;
use std::default;
use std::rc::Rc;
use {Attributes, Flavor, Loc, RpChannel, RpPathSpec, Translate, Translator};

#[derive(Debug, Clone, Serialize)]
pub enum RpHttpMethod {
    Get,
    Post,
    Put,
    Update,
    Delete,
    Patch,
    Head,
}

impl RpHttpMethod {
    /// Treat this method to an all uppercase string representing the method.
    pub fn as_str(&self) -> &str {
        use self::RpHttpMethod::*;

        match *self {
            Get => "GET",
            Post => "POST",
            Put => "PUT",
            Update => "UPDATE",
            Delete => "DELETE",
            Patch => "PATCH",
            Head => "HEAD",
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
#[serde(bound = "F: ::serde::Serialize, F::Type: ::serde::Serialize")]
pub struct RpEndpointHttp<F: 'static>
where
    F: Flavor,
{
    /// Path specification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<RpPathSpec<F>>,
    /// Argument that is the body of the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<RpEndpointArgument<F>>,
    /// HTTP method.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<RpHttpMethod>,
    /// Accepted media types.
    pub accept: RpAccept,
}

impl<F: 'static, T> Translate<T> for RpEndpointHttp<F>
where
    F: Flavor,
    T: Translator<Source = F>,
{
    type Source = F;
    type Out = RpEndpointHttp<T::Target>;

    /// Translate into different flavor.
    fn translate(self, translator: &T) -> Result<RpEndpointHttp<T::Target>> {
        Ok(RpEndpointHttp {
            path: self.path.translate(translator)?,
            body: self.body.translate(translator)?,
            method: self.method,
            accept: self.accept,
        })
    }
}

/// An argument to an endpont.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(bound = "F::Type: ::serde::Serialize")]
pub struct RpEndpointArgument<F: 'static>
where
    F: Flavor,
{
    /// Identifier of the argument.
    pub ident: Rc<Loc<String>>,
    /// Safe identifier for the argument.
    pub safe_ident: Rc<Option<String>>,
    /// Channel of the argument.
    pub channel: Loc<RpChannel<F>>,
}

impl<F: 'static, T> Translate<T> for RpEndpointArgument<F>
where
    F: Flavor,
    T: Translator<Source = F>,
{
    type Source = F;
    type Out = RpEndpointArgument<T::Target>;

    /// Translate into different flavor.
    fn translate(self, translator: &T) -> Result<RpEndpointArgument<T::Target>> {
        Ok(RpEndpointArgument {
            ident: self.ident,
            safe_ident: self.safe_ident,
            channel: self.channel.translate(translator)?,
        })
    }
}

impl<F: 'static> RpEndpointArgument<F>
where
    F: Flavor,
{
    /// Access the actual identifier of the endpoint argument.
    pub fn ident(&self) -> &str {
        self.ident.as_str()
    }

    /// Access the safe identifier for the endpoint argument.
    pub fn safe_ident(&self) -> &str {
        Option::as_ref(&self.safe_ident)
            .map(|s| s.as_str())
            .unwrap_or_else(|| self.ident.as_str())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(bound = "F: ::serde::Serialize, F::Type: ::serde::Serialize")]
pub struct RpEndpoint<F: 'static>
where
    F: Flavor,
{
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
    pub arguments: Vec<RpEndpointArgument<F>>,
    /// Request type that this endpoint accepts with.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<RpEndpointArgument<F>>,
    /// Response type that this endpoint responds with.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<Loc<RpChannel<F>>>,
    /// HTTP configuration.
    pub http: RpEndpointHttp<F>,
}

impl<F: 'static> RpEndpoint<F>
where
    F: Flavor,
{
    pub fn id_parts<T>(&self, filter: T) -> Vec<String>
    where
        T: Fn(&str) -> String,
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

impl<F: 'static, T> Translate<T> for RpEndpoint<F>
where
    F: Flavor,
    T: Translator<Source = F>,
{
    type Source = F;
    type Out = RpEndpoint<T::Target>;

    /// Translate into different flavor.
    fn translate(self, translator: &T) -> Result<RpEndpoint<T::Target>> {
        Ok(RpEndpoint {
            ident: self.ident,
            safe_ident: self.safe_ident,
            name: self.name,
            comment: self.comment,
            attributes: self.attributes,
            arguments: self.arguments.translate(translator)?,
            request: self.request.translate(translator)?,
            response: self.response.translate(translator)?,
            http: self.http.translate(translator)?,
        })
    }
}

/// A model that describes the endpoint as an HTTP/1.1 endpoint.
#[derive(Debug, Clone)]
pub struct RpEndpointHttp1<F: 'static>
where
    F: Flavor,
{
    pub request: Option<F::Type>,
    pub response: Option<F::Type>,
    pub path: RpPathSpec<F>,
    pub method: RpHttpMethod,
}

impl<F: 'static> RpEndpointHttp1<F>
where
    F: Clone + Flavor,
{
    /// Convert the general HTTP information into HTTP/1.1 if applicable.
    pub fn from_endpoint(endpoint: &RpEndpoint<F>) -> Option<RpEndpointHttp1<F>> {
        use self::RpChannel::*;

        // HTTP/1.1 requires a path.
        let path = match endpoint.http.path.as_ref() {
            Some(path) => path.clone(),
            None => return None,
        };

        let request_ty = endpoint.request.as_ref().map(|r| Loc::borrow(&r.channel));
        let response_ty = endpoint.response.as_ref().map(|r| Loc::borrow(r));

        let (request, response) = match (request_ty, response_ty) {
            (Some(&Unary { ty: ref request }), Some(&Unary { ty: ref response })) => {
                (Some(request.clone()), Some(response.clone()))
            }
            (None, Some(&Unary { ty: ref response })) => (None, Some(response.clone())),
            (Some(&Unary { ty: ref request }), None) => (Some(request.clone()), None),
            (None, None) => (None, None),
            _ => return None,
        };

        let method = endpoint
            .http
            .method
            .as_ref()
            .cloned()
            .unwrap_or(RpHttpMethod::Get);

        return Some(RpEndpointHttp1 {
            request: request,
            response: response,
            path: path,
            method: method,
        });
    }
}
