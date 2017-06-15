#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum RpModifier {
    Required,
    Optional,
    Repeated,
}
