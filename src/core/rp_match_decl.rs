use super::errors::*;
use super::rp_by_type_match::RpByTypeMatch;
use super::rp_by_value_match::RpByValueMatch;
use super::rp_loc::RpLoc;
use super::rp_match_condition::RpMatchCondition;
use super::rp_match_kind::RpMatchKind;
use super::rp_match_member::RpMatchMember;
use super::rp_match_variable::RpMatchVariable;
use super::rp_type::RpType;
use super::rp_value::RpValue;

#[derive(Debug, Clone, Serialize)]
pub struct RpMatchDecl {
    pub by_value: Vec<(RpLoc<RpValue>, RpByValueMatch)>,
    pub by_type: Vec<(RpMatchKind, RpByTypeMatch)>,
}

impl RpMatchDecl {
    pub fn new() -> RpMatchDecl {
        RpMatchDecl {
            by_value: Vec::new(),
            by_type: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.by_value.is_empty() && self.by_type.is_empty()
    }

    pub fn identify_match_kind(&self, variable: &RpMatchVariable) -> RpMatchKind {
        match variable.ty {
            RpType::Double |
            RpType::Float |
            RpType::Signed(_) |
            RpType::Unsigned(_) => RpMatchKind::Number,
            RpType::Boolean => RpMatchKind::Boolean,
            RpType::String | RpType::Bytes => RpMatchKind::String,
            RpType::Any => RpMatchKind::Any,
            RpType::Name(_) |
            RpType::Map(_, _) => RpMatchKind::Object,
            RpType::Array(_) => RpMatchKind::Array,
        }
    }

    pub fn push(&mut self, member: RpLoc<RpMatchMember>) -> Result<()> {
        match member.condition.inner {
            RpMatchCondition::Type(ref variable) => {
                let match_kind = self.identify_match_kind(variable);

                {
                    // conflicting when type matches
                    let result =
                        self.by_type.iter().find(|e| e.0 == match_kind || e.0 == RpMatchKind::Any);

                    if let Some(&(_, ref existing_value)) = result {
                        let err = ErrorKind::MatchConflict(member.condition.pos.clone(),
                                                           existing_value.instance.pos.clone());
                        return Err(err.into());
                    }
                }

                self.by_type.push((match_kind,
                                   RpByTypeMatch {
                                       variable: variable.clone(),
                                       instance: member.value.clone(),
                                   }));
            }
            RpMatchCondition::Value(ref value) => {
                {
                    // conflicting when value matches
                    let result = self.by_value.iter().find(|e| e.0.inner == value.inner);

                    if let Some(&(_, ref existing_value)) = result {
                        let err = ErrorKind::MatchConflict(member.condition.pos.clone(),
                                                           existing_value.instance.pos.clone());
                        return Err(err.into());
                    }
                }

                self.by_value
                    .push((value.clone(), RpByValueMatch { instance: member.value.clone() }));
            }
        }

        Ok(())
    }
}
