use std::{collections::HashMap, sync::Arc};

use actix::prelude::*;
use tokio::{net::unix::pipe::Receiver, sync::Mutex};

use crate::{EndPoint, EthernetFrame, OnReceive};

pub type Uuid = usize;

#[derive(Debug, Clone)]
pub struct Core {
    addr: Addr<CoreRaw>,
}

impl Core {
    pub fn new() -> Self {
        let core_raw = CoreRaw::new();

        Core {
            addr: core_raw.start(),
        }
    }

    pub async fn create_ep(&self, on_receive: Recipient<OnReceive>) -> EndPoint {
        self.addr.send(CreateEndPoint { on_receive }).await.unwrap()
    }

    pub fn send(&self, uuid: Uuid, payload: EthernetFrame) {
        self.addr.do_send(Send { uuid, payload });
    }

    pub async fn connect<E0: Connectable, E1: Connectable>(&self, e0: &E0, e1: &E1) {
        self.addr.do_send(Connect {
            e0: e0.uuid().await,
            e1: e1.uuid().await,
        });
    }
}

#[derive(Debug)]
struct CoreRaw {
    next_uuid: Uuid,
    endpoints: Arc<Mutex<HashMap<Uuid, EndPoint>>>,
    connections: Arc<Mutex<HashMap<Uuid, Vec<Uuid>>>>,
}

impl Actor for CoreRaw {
    type Context = Context<Self>;
}

impl CoreRaw {
    fn new() -> Self {
        CoreRaw {
            next_uuid: 0,
            endpoints: Arc::new(Mutex::new(HashMap::new())),
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[derive(Message, Debug)]
#[rtype(EndPoint)]
struct CreateEndPoint {
    on_receive: Recipient<OnReceive>,
}

impl Handler<CreateEndPoint> for CoreRaw {
    type Result = ResponseFuture<EndPoint>;

    fn handle(&mut self, msg: CreateEndPoint, ctx: &mut Self::Context) -> Self::Result {
        let endpoints = self.endpoints.clone();
        let me = ctx.address();
        let uuid = self.next_uuid;
        self.next_uuid += 1;

        Box::pin(async move {
            // TODO: Coreが持つのとUserに持たせるepは違うものにする
            let ep = EndPoint::new(Core { addr: me }, uuid, msg.on_receive);
            endpoints.lock().await.insert(uuid, ep.clone());
            ep
        })
    }
}

#[derive(Message, MessageResponse, Debug)]
#[rtype(result = "()")]
struct Send {
    pub uuid: Uuid,
    pub payload: EthernetFrame,
}

impl Handler<Send> for CoreRaw {
    type Result = ResponseFuture<()>;

    fn handle(&mut self, msg: Send, ctx: &mut Self::Context) -> Self::Result {
        let connections = self.connections.clone();
        let endpoints = self.endpoints.clone();

        Box::pin(async move {
            if let Some(targets) = connections.lock().await.get(&msg.uuid) {
                for target in targets.iter() {
                    if let Some(target_ep) = endpoints.lock().await.get_mut(target) {
                        target_ep.write(msg.payload.clone()).await;
                    }
                }
            }
        })
    }
}

#[derive(Message, Debug)]
#[rtype(result = "()")]
struct Connect {
    e0: Uuid,
    e1: Uuid,
}

impl Handler<Connect> for CoreRaw {
    type Result = ResponseFuture<()>;

    fn handle(&mut self, msg: Connect, ctx: &mut Self::Context) -> Self::Result {
        let connections = self.connections.clone();

        Box::pin(async move {
            connections
                .lock()
                .await
                .entry(msg.e0)
                .and_modify(|tbl| tbl.push(msg.e1))
                .or_insert(vec![msg.e1]);

            connections
                .lock()
                .await
                .entry(msg.e1)
                .and_modify(|tbl| tbl.push(msg.e0))
                .or_insert(vec![msg.e0]);
        })
    }
}

pub trait Connectable {
    async fn uuid(&self) -> Uuid;
}
