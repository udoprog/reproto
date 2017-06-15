use super::rp_type::RpType;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum RpPathFragment {
    Variable { name: String, ty: RpType },
}
