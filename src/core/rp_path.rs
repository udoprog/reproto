use super::rp_path_fragment::RpPathFragment;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct RpPath {
    pub fragments: Vec<RpPathFragment>,
}
