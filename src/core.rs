use ractor::{Actor, ActorProcessingErr, ActorRef, DerivedActorRef, RpcReplyPort, call, cast};
use std::collections::HashMap;

use crate::{EndPoint, EndPointMsg, EthernetFrame, OnReceiveRaw};

pub type Uuid = usize;

#[derive(Clone, Debug)]
pub struct Core {
    addr: ActorRef<CoreMsg>,
}

impl Core {
    #[tracing::instrument(target = "core", level = "info")]
    pub fn new(actor_ref: ActorRef<CoreMsg>) -> Self {
        Core { addr: actor_ref }
    }

    #[tracing::instrument(target = "core", level = "info")]
    pub async fn spawn() -> Self {
        tracing::info!("Spawning Core");
        let (addr, _) = CoreActor::spawn(None, CoreActor, ()).await.unwrap();
        tracing::info!("Core spawned: addr={:?}", addr);
        Core { addr }
    }

    #[tracing::instrument(target = "core", level = "debug", skip(self, on_receive_raw))]
    pub async fn create_ep(&self, on_receive_raw: DerivedActorRef<OnReceiveRaw>) -> EndPoint {
        tracing::debug!("Core create_ep");
        call!(self.addr, CoreMsg::CreateEndPoint, on_receive_raw).unwrap()
    }

    #[tracing::instrument(target = "core", level = "debug", skip(self, payload))]
    pub fn send(&self, uuid: Uuid, payload: EthernetFrame) {
        tracing::debug!("Core send: uuid={:?} payload={:?}", uuid, payload);
        cast!(self.addr, CoreMsg::Send(uuid, payload)).unwrap()
    }

    #[tracing::instrument(target = "core", level = "info", skip(self))]
    pub async fn connect(&self, e0: Uuid, e1: Uuid) {
        tracing::info!("Core connect: e0={:?} <-> e1={:?}", e0, e1);
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

    #[tracing::instrument(target = "core_actor", level = "info", skip_all)]
    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _args: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        tracing::info!("CoreActor pre_start");
        Ok(CoreActorState {
            next_uuid: 0,
            endpoints: HashMap::new(),
            connections: HashMap::new(),
        })
    }

    #[tracing::instrument(target = "core_actor", level = "debug", skip_all)]
    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        tracing::trace!("CoreActor handle: msg={:?}", msg);
        match msg {
            CoreMsg::CreateEndPoint(on_receive_raw, reply) => {
                let uuid = state.next_uuid;
                state.next_uuid += 1;
                tracing::info!("CoreActor: CreateEndPoint uuid={:?}", uuid);
                let core = Core {
                    addr: _myself.clone(),
                };
                let ep = EndPoint::new(EndPoint::spawn(core, uuid, on_receive_raw).await);
                state.endpoints.insert(uuid, ep.clone());
                reply.send(ep).unwrap();
            },
            CoreMsg::Send(uuid, payload) => {
                tracing::debug!("CoreActor: Send uuid={:?} payload={:?}", uuid, payload);
                if let Some(targets) = state.connections.get(&uuid) {
                    for target in targets {
                        if let Some(ep) = state.endpoints.get(target) {
                            ep.write(payload.clone());
                        }
                    }
                }
            },
            CoreMsg::Connect(e0, e1) => {
                tracing::info!("CoreActor: Connect e0={:?} <-> e1={:?}", e0, e1);
                state.connections.entry(e0).or_default().push(e1);
                state.connections.entry(e1).or_default().push(e0);
            },
        }
        Ok(())
    }
}
