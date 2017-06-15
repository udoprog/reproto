use parser::ast;
use std::rc::Rc;
use super::errors::*;
use super::into_model::IntoModel;
use super::merge::Merge;
use super::rp_loc::RpPos;

#[derive(Debug, Clone, Serialize)]
pub struct RpServiceBody {
    pub name: String,
    pub comment: Vec<String>,
}

impl IntoModel for ast::ServiceBody {
    type Output = Rc<RpServiceBody>;

    fn into_model(self, _pos: &RpPos) -> Result<Rc<RpServiceBody>> {
        let service_body = RpServiceBody {
            name: self.name,
            comment: self.comment,
        };

        Ok(Rc::new(service_body))
    }
}

impl Merge for RpServiceBody {
    fn merge(&mut self, _source: RpServiceBody) -> Result<()> {
        Ok(())
    }
}
