use ractor::{Actor, ActorProcessingErr, ActorRef, DerivedActorRef, RpcReplyPort, call, cast};
use std::collections::HashMap;
use tracing::instrument;

use crate::{EndPoint, EndPointMsg, EthernetFrame, OnReceiveRaw};

pub type Uuid = usize;

#[derive(Clone, Debug)]
pub struct Core {
    addr: ActorRef<CoreMsg>,
}

impl Core {
    #[instrument(skip(actor_ref))]
    pub fn new(actor_ref: ActorRef<CoreMsg>) -> Self {
        Core { addr: actor_ref }
    }

    #[instrument()]
    pub async fn spawn() -> Self {
        let (addr, _) = CoreActor::spawn(None, CoreActor, ()).await.unwrap();
        Core { addr }
    }

    #[instrument(skip(self, on_receive_raw))]
    pub async fn create_ep(&self, on_receive_raw: DerivedActorRef<OnReceiveRaw>) -> EndPoint {
        tracing::info!("create_ep called");
        call!(self.addr, CoreMsg::CreateEndPoint, on_receive_raw).unwrap()
    }

    #[instrument(skip(self, payload))]
    pub fn send(&self, uuid: Uuid, payload: EthernetFrame) {
        tracing::info!(uuid = ?uuid, "send called");
        cast!(self.addr, CoreMsg::Send(uuid, payload)).unwrap()
    }

    #[instrument(skip(self))]
    pub async fn connect(&self, e0: Uuid, e1: Uuid) {
        tracing::info!(e0 = ?e0, e1 = ?e1, "connect called");
        cast!(self.addr, CoreMsg::Connect(e0, e1)).unwrap()
    }
}

#[derive(Debug)]
pub enum CoreMsg {
    CreateEndPoint(DerivedActorRef<OnReceiveRaw>, RpcReplyPort<EndPoint>),
    Send(Uuid, EthernetFrame),
    Connect(Uuid, Uuid),
}

#[derive(Debug)]
struct CoreActor;

#[derive(Debug)]
struct CoreActorState {
    next_uuid: Uuid,
    endpoints: HashMap<Uuid, EndPoint>,
    connections: HashMap<Uuid, Vec<Uuid>>,
}

#[ractor::async_trait]
impl Actor for CoreActor {
    type Msg = CoreMsg;
    type State = CoreActorState;
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _args: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(CoreActorState {
            next_uuid: 0,
            endpoints: HashMap::new(),
            connections: HashMap::new(),
        })
    }

    #[instrument(skip(self, _myself, msg, state))]
    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match &msg {
            CoreMsg::CreateEndPoint(_, _) => {
                tracing::info!("handle CreateEndPoint");
            },
            CoreMsg::Send(uuid, _) => {
                tracing::info!(uuid = ?uuid, "handle Send");
            },
            CoreMsg::Connect(e0, e1) => {
                tracing::info!(e0 = ?e0, e1 = ?e1, "handle Connect");
            },
        }
        match msg {
            CoreMsg::CreateEndPoint(on_receive_raw, reply) => {
                let uuid = state.next_uuid;
                state.next_uuid += 1;
                let core = Core {
                    addr: _myself.clone(),
                };
                let ep = EndPoint::new(EndPoint::spawn(core, uuid, on_receive_raw).await);
                state.endpoints.insert(uuid, ep.clone());
                reply.send(ep).unwrap();
            },
            CoreMsg::Send(uuid, payload) => {
                if let Some(targets) = state.connections.get(&uuid) {
                    for target in targets {
                        if let Some(ep) = state.endpoints.get(target) {
                            ep.write(payload.clone());
                        }
                    }
                }
            },
            CoreMsg::Connect(e0, e1) => {
                state.connections.entry(e0).or_default().push(e1);
                state.connections.entry(e1).or_default().push(e0);
            },
        }
        Ok(())
    }
}
