use std::rc::Rc;
use {EmptyObject, Object};

#[derive(Clone, Debug, Serialize)]
pub struct Pos {
    #[serde(skip)]
    pub object: Rc<Box<Object>>,
    pub start: usize,
    pub end: usize,
}

impl Pos {
    pub fn empty() -> Pos {
        Pos {
            object: Rc::new(Box::new(EmptyObject::new("empty"))),
            start: 0,
            end: 0,
        }
    }
}

impl From<(Rc<Box<Object>>, usize, usize)> for Pos {
    fn from(value: (Rc<Box<Object>>, usize, usize)) -> Self {
        Pos {
            object: value.0,
            start: value.1,
            end: value.2,
        }
    }
}
