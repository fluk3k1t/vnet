use crate::{Actor, Caller, Context, Core, Handler, Message, Uuid};

#[derive(Debug)]
pub struct Com {
    uuid: Uuid,
    core_caller: Caller<Core>,
}

impl Com {
    pub fn new(uuid: Uuid, core_caller: Caller<Core>) -> Self {
        Com { uuid, core_caller }
    }
}

impl Actor for Com {
    type Context = Self;
}

pub struct Send {
    payload: String,
}

impl Message for Send {
    type Return = ();
}

impl Handler<Send> for Com {
    async fn handle(&mut self, m: Send, ctx: &mut Context<Self>) -> <Send as Message>::Return {
        self.core_caller
            .call(crate::Greet {
                content: "from com".to_string(),
            })
            .await;
    }
}
