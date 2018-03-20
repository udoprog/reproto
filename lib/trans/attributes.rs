//! Handle parsing of attributes.

use core::{Attributes, Context, Loc, Pos, RpAccept, RpChannel, RpEndpointArgument, RpEndpointHttp,
           RpHttpMethod, RpPathSpec, RpType, RpValue, WithPos};
use core::errors::Result;
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
    let mut unused_args = arguments
        .iter()
        .map(|a| (a.ident.as_str(), Loc::pos(&a.ident)))
        .collect::<HashMap<_, _>>();

    if let Some(path) = selection.take("path") {
        let (path, pos) = Loc::take_pair(path);
        http.path = Some(parse_path(scope, path, &mut unused_args).with_pos(pos)?);
    }

    if let Some(body) = selection.take("body") {
        let (body, pos) = Loc::take_pair(body);
        let body = body.as_identifier().with_pos(&pos)?;

        if unused_args.remove(body).is_none() {
            return Err(
                ctx.report()
                    .err(
                        pos,
                        format!("body `{}` is not an argument to endpoint", body),
                    )
                    .into(),
            );
        }

        http.body = Some(body.to_string());
    }

    if let Some(method) = selection.take("method") {
        let (method, pos) = Loc::take_pair(method);
        http.method = Some(parse_method(method).with_pos(pos)?);
    }

    if let Some(accept) = selection.take("accept") {
        let accept = Loc::and_then(accept, |a| {
            a.as_string().and_then(|a| match a {
                "application/json" => Ok(RpAccept::Json),
                "text/plain" => Ok(RpAccept::Text),
                _ => Err("unsupported media type".into()),
            })
        })?;

        http_verify_accept(ctx, &accept, response)?;
        http.accept = Loc::take(accept);
    }

    // Assert that all arguments are used somehow.
    if !unused_args.is_empty() {
        let mut report = ctx.report();

        for pos in unused_args.values() {
            report = report.err(*pos, "Argument not used in #[http(...)] attribute");
        }

        return Err(report.into());
    }

    check_selection!(scope.ctx(), selection);
    return Ok(http);

    /// Parse a path specification.
    fn parse_path(
        scope: &Scope,
        path: RpValue,
        unused_args: &mut HashMap<&str, &Pos>,
    ) -> Result<RpPathSpec> {
        let path = path.as_string()?;
        let path = path_parser::parse(path).map_err(|e| {
            format!("Bad path: {}: {}", path, e.display())
        })?;
        let path = path.into_model(scope)?;

        for var in path.vars() {
            if unused_args.remove(var).is_none() {
                return Err(
                    format!("path variable `{}` is not an argument to endpoint", var).into(),
                );
            }
        }

        Ok(path)
    }

    /// Parse a method.
    fn parse_method(method: RpValue) -> Result<RpHttpMethod> {
        use self::RpHttpMethod::*;

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
            ref accept if *accept == RpAccept::Json => return Ok(()),
            _ => {
                if *response.ty() == RpType::String {
                    return Ok(());
                }

                return Err(
                    ctx.report()
                        .err(
                            Loc::pos(response),
                            "Only `string` responses are supported for the given `accept`",
                        )
                        .info(pos, "Specified here")
                        .into(),
                );
            }
        }
    }
}

/// `#[import(..)]` attributes
pub fn import(scope: &Scope, attributes: &mut Attributes) -> Result<Vec<Loc<String>>> {
    let mut out = Vec::new();

    if let Some(imports) = attributes.take_all_selections("import") {
        let ctx = scope.ctx();

        for mut import in imports {
            for word in import.take_words() {
                out.push(Loc::and_then(
                    word,
                    |w| w.as_str().map(ToString::to_string),
                )?);
            }

            check_selection!(ctx, import);
        }
    }

    Ok(out)
}
