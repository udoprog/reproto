//! Model for sub-types

use super::{Loc, RpCode, RpDecl, RpField, RpName};

#[derive(Debug, Clone, Serialize)]
pub struct RpSubType {
    pub name: RpName,
    pub ident: String,
    pub comment: Vec<String>,
    /// Inner declarations.
    pub decls: Vec<RpDecl>,
    pub fields: Vec<Loc<RpField>>,
    pub codes: Vec<Loc<RpCode>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_type_name: Option<Loc<String>>,
}

impl RpSubType {
    pub fn name(&self) -> &str {
        self.sub_type_name.as_ref().map(|t| t.as_str()).unwrap_or(
            &self.ident,
        )
    }
}
