use super::rp_loc::RpLoc;
use super::rp_match_variable::RpMatchVariable;
use super::rp_value::RpValue;

#[derive(Debug, Clone, Serialize)]
pub struct RpByTypeMatch {
    pub variable: RpLoc<RpMatchVariable>,
    pub instance: RpLoc<RpValue>,
}
