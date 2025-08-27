use crate::{Actor, Handler, Message};

pub type Uuid = usize;

pub struct Core {
    next_uuid: Uuid,
}

impl Core {
    pub fn new() -> Self {
        Core { next_uuid: 0 }
    }
}

impl Message for CreateCom {
    type Return = String;
}

pub struct CreateCom;

impl Actor for Core {
    type Context = Self;
}

impl Handler<CreateCom> for Core {
    fn handle(&mut self, m: CreateCom) -> String {
        "hello".to_string()
    }
}
