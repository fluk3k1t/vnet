use crate::{Actor, Caller, Context, Core, Handler, Message, ResponseFuture, Uuid};

// #[derive(Debug)]
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
    pub payload: String,
}

impl Message for Send {
    type Return = ResponseFuture<String>;
}

impl Handler<Send> for Com {
    fn handle(&mut self, m: Send, ctx: &mut Context<Self>) -> ResponseFuture<String> {
        let mut core_caller = self.core_caller.clone();
        Box::pin(async move {
            let r = core_caller
                .call(crate::Greet {
                    content: "from com".to_string(),
                })
                .await;

            "ok from com".to_string()
        })
    }
}
