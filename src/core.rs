use std::collections::HashMap;

use crate::{Actor, Caller, Com, Context, Handler, Message};

pub type Uuid = usize;

#[derive(Debug)]
pub struct Core {
    coms: HashMap<Uuid, Caller<Com>>,
    next_uuid: Uuid,
}

impl Core {
    pub fn new() -> Self {
        Core {
            coms: HashMap::new(),
            next_uuid: 0,
        }
    }
}

impl Handler<CreateCom> for Core {
    fn handle(&mut self, m: CreateCom, ctx: &mut Context<Self>) -> Caller<Com> {
        let (com, _) = Com::new(self.next_uuid, ctx.caller()).start();
        self.coms.insert(self.next_uuid, com.clone());

        self.next_uuid += 1;

        com
    }
}

impl Handler<Greet> for Core {
    fn handle(&mut self, m: Greet, ctx: &mut Context<Self>) -> <Greet as Message>::Return {
        println!("core greet {}", m.content);
    }
}

impl Actor for Core {
    type Context = Self;
}

pub struct CreateCom;

impl Message for CreateCom {
    type Return = Caller<Com>;
}

pub struct Greet {
    pub content: String,
}

impl Message for Greet {
    type Return = ();
}
