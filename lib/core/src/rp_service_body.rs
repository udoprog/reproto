//! Model for services.

use super::{Flavor, Loc, Translate, Translator};
use errors::Result;

#[derive(Debug, Clone, Serialize, Default)]
pub struct RpServiceBodyHttp {
    /// Default URL to use for service.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<Loc<String>>,
}

decl_body!(pub struct RpServiceBody<F> {
    pub http: RpServiceBodyHttp,
    pub endpoints: Vec<Loc<F::Endpoint>>,
});

impl<F: 'static, T> Translate<T> for RpServiceBody<F>
where
    F: Flavor,
    T: Translator<Source = F>,
{
    type Source = F;
    type Out = RpServiceBody<T::Target>;

    /// Translate into different flavor.
    fn translate(self, translator: &T) -> Result<RpServiceBody<T::Target>> {
        translator.visit(&self.name)?;

        let endpoints = self.endpoints
            .into_iter()
            .map(|e| Loc::and_then(e, |e| translator.translate_endpoint(e)))
            .collect::<Result<Vec<_>>>()?;

        Ok(RpServiceBody {
            name: self.name,
            ident: self.ident,
            comment: self.comment,
            decls: self.decls.translate(translator)?,
            http: self.http,
            endpoints: endpoints,
        })
    }
}
