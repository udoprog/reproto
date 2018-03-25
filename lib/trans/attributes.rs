//! Handle parsing of attributes.

use core::errors::Result;
use core::flavored::{RpAccept, RpChannel, RpEndpointArgument, RpEndpointHttp, RpHttpMethod,
                     RpPathSpec, RpValue};
use core::{self, Attributes, Context, Loc, Pos, WithPos};
use into_model::IntoModel;
use path_parser;
use scope::Scope;
use std::collections::HashMap;

/// `#[reserved(..)]` attribute.
pub fn reserved(scope: &Scope, attributes: &mut Attributes) -> Result<HashMap<String, Pos>> {
    let mut reserved: HashMap<String, Pos> = HashMap::new();

    let selection = match attributes.take_selection("reserved") {
        None => return Ok(reserved),
        Some(selection) => selection,
    };

    let (mut selection, _pos) = Loc::take_pair(selection);

    for word in selection.take_words() {
        let (field, pos) = Loc::take_pair(word);
        let field = field.as_string().map(|id| id.to_string()).with_pos(&pos)?;
        reserved.insert(field, pos);
    }

    check_selection!(scope.ctx(), selection);

    Ok(reserved)
}

/// `#[http(..)]` attribute for endpoints.
pub fn endpoint_http(
    scope: &Scope,
    attributes: &mut Attributes,
    request: &mut Option<RpEndpointArgument>,
    response: Option<&Loc<RpChannel>>,
    arguments: &Vec<RpEndpointArgument>,
) -> Result<RpEndpointHttp> {
    let mut http = RpEndpointHttp::default();

    let selection = match attributes.take_selection("http") {
        None => return Ok(http),
        Some(selection) => selection,
    };

    let ctx = scope.ctx();

    let (mut selection, _pos) = Loc::take_pair(selection);

    // Keep track of used variables.
    let mut args = arguments
        .iter()
        .map(|a| (a.ident(), a))
        .collect::<HashMap<_, _>>();

    if let Some(path) = selection.take("path") {
        let (path, pos) = Loc::take_pair(path);
        http.path = Some(parse_path(scope, path, &mut args).with_pos(pos)?);
    }

    if let Some(method) = selection.take("method") {
        let (method, pos) = Loc::take_pair(method);
        http.method = Some(parse_method(method).with_pos(pos)?);
    }

    if let Some(accept) = selection.take("accept") {
        let accept = Loc::and_then(accept, |a| {
            a.as_string().and_then(|a| match a {
                "application/json" => Ok(core::RpAccept::Json),
                "text/plain" => Ok(core::RpAccept::Text),
                _ => Err("unsupported media type".into()),
            })
        })?;

        http_verify_accept(ctx, &accept, response)?;
        http.accept = Loc::take(accept);
    }

    // All arguments used, no request body.
    if args.is_empty() {
        *request = None;
    }

    // Assert that all arguments are used somehow.
    if !args.is_empty() {
        let mut report = ctx.report();

        for arg in args.values() {
            if let Some(ref mut request) = request.as_mut() {
                if arg.ident == request.ident {
                    continue;
                }
            }

            report = report.err(
                Loc::pos(&arg.ident),
                "Argument not used in #[http(...)] attribute",
            );
        }

        if !report.is_empty() {
            return Err(report.into());
        }
    }

    check_selection!(scope.ctx(), selection);
    return Ok(http);

    /// Parse a path specification.
    fn parse_path<'a, 'b: 'a>(
        scope: &Scope,
        path: RpValue,
        args: &'a mut HashMap<&'b str, &'b RpEndpointArgument>,
    ) -> Result<RpPathSpec> {
        let path = path.as_string()?;
        let path =
            path_parser::parse(path).map_err(|e| format!("Bad path: {}: {}", path, e.display()))?;
        let path = (args, path).into_model(scope)?;
        Ok(path)
    }

    /// Parse a method.
    fn parse_method(method: RpValue) -> Result<RpHttpMethod> {
        use core::RpHttpMethod::*;

        let m = match method.as_string()? {
            "GET" => GET,
            "POST" => POST,
            "PUT" => PUT,
            "UPDATE" => UPDATE,
            "DELETE" => DELETE,
            "PATCH" => PATCH,
            "HEAD" => HEAD,
            method => return Err(format!("no such method: {}", method).into()),
        };

        Ok(m)
    }

    /// Check that accept matches response.
    fn http_verify_accept(
        ctx: &Context,
        accept: &Loc<RpAccept>,
        response: Option<&Loc<RpChannel>>,
    ) -> Result<()> {
        let response = match response {
            Some(response) => response,
            None => return Ok(()),
        };

        let (accept, pos) = Loc::borrow_pair(&accept);

        match *accept {
            // Can handle complex data types.
            ref accept if *accept == core::RpAccept::Json => return Ok(()),
            _ => {
                if *response.ty() == core::RpType::String {
                    return Ok(());
                }

                return Err(ctx.report()
                    .err(
                        Loc::pos(response),
                        "Only `string` responses are supported for the given `accept`",
                    )
                    .info(pos, "Specified here")
                    .into());
            }
        }
    }
}

/// `#[import(..)]` attributes
pub fn import(scope: &Scope, attributes: &mut Attributes) -> Result<Vec<Loc<String>>> {
    let mut out = Vec::new();

    if let Some(mut imports) = attributes.take_selection("import") {
        let ctx = scope.ctx();

        for import in imports.take_words() {
            out.push(Loc::and_then(import, |w| {
                w.as_str().map(ToString::to_string)
            })?);
        }

        check_selection!(ctx, imports);
    }

    Ok(out)
}
