use super::rp_loc::RpLoc;
use super::rp_value::RpValue;

#[derive(Debug, Clone, Serialize)]
pub struct RpByValueMatch {
    pub instance: RpLoc<RpValue>,
}
