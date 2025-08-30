use actix::{dev::MessageResponse, prelude::*};
use tracing::debug;
use tracing_subscriber::field::debug;

use crate::{Core, EthernetFrame, Uuid};

#[derive(MessageResponse, Clone, Debug)]
pub struct EndPoint {
    addr: Addr<EndPointRaw>,
}

impl EndPoint {
    pub fn new(core: Core, uuid: Uuid, on_receive: Recipient<OnReceive>) -> Self {
        let ep_raw = EndPointRaw::new(core, uuid, on_receive);
        EndPoint {
            addr: ep_raw.start(),
        }
    }

    pub async fn uuid(&self) -> Uuid {
        self.addr.send(GetUuid).await.unwrap()
    }

    pub fn send(&self, payload: EthernetFrame) {
        self.addr.do_send(Send(payload));
    }

    pub async fn write(&self, payload: EthernetFrame) {
        self.addr.send(Write(payload)).await.unwrap();
    }
}

#[derive(MessageResponse)]
struct EndPointRaw {
    uuid: Uuid,
    core: Core,
    on_receive: Recipient<OnReceive>,
}

impl Actor for EndPointRaw {
    type Context = Context<Self>;
}

impl EndPointRaw {
    fn new(core: Core, uuid: Uuid, on_receive: Recipient<OnReceive>) -> Self {
        EndPointRaw {
            uuid,
            core,
            on_receive,
        }
    }
}

#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct OnReceive {
    pub dst: Uuid,
    pub payload: EthernetFrame,
}

#[derive(Message)]
#[rtype(Uuid)]
struct GetUuid;

impl Handler<GetUuid> for EndPointRaw {
    type Result = Uuid;
    fn handle(&mut self, msg: GetUuid, ctx: &mut Self::Context) -> Self::Result {
        self.uuid
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct Send(EthernetFrame);

impl Handler<Send> for EndPointRaw {
    type Result = ();

    fn handle(&mut self, msg: Send, ctx: &mut Self::Context) -> Self::Result {
        let core = self.core.clone();
        let uuid = self.uuid;

        core.send(uuid, msg.0);
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct Write(EthernetFrame);

impl Handler<Write> for EndPointRaw {
    type Result = ResponseFuture<()>;

    fn handle(&mut self, msg: Write, ctx: &mut Self::Context) -> Self::Result {
        let on_receive = self.on_receive.clone();
        let uuid = self.uuid;

        Box::pin(async move {
            on_receive.do_send(OnReceive {
                payload: msg.0,
                dst: uuid,
            });
        })
    }
}
