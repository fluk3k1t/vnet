use crate::{Core, EthernetFrame, Uuid};
use ractor::{Actor, ActorProcessingErr, ActorRef, DerivedActorRef, RpcReplyPort, call, cast};
use tracing::instrument;

#[derive(Debug, Clone)]
pub struct OnReceiveRaw {
    pub dst: Uuid,
    pub payload: EthernetFrame,
}

#[derive(Debug)]
pub enum EndPointMsg {
    GetUuid(RpcReplyPort<Uuid>),
    Send(EthernetFrame),
    Write(EthernetFrame),
}

#[derive(Debug, Clone)]
pub struct EndPoint {
    addr: ActorRef<EndPointMsg>,
}

impl EndPoint {
    #[tracing::instrument(target = "endpoint", level = "info", skip_all)]
    pub fn new(addr: ActorRef<EndPointMsg>) -> Self {
        EndPoint { addr }
    }

    #[instrument]
    #[tracing::instrument(target = "endpoint", level = "trace", skip(self))]
    pub async fn uuid(&self) -> Uuid {
        let uuid = call!(self.addr, EndPointMsg::GetUuid).unwrap();
        tracing::trace!("EndPoint uuid: {:?}", uuid);
        uuid
    }

    #[tracing::instrument(target = "endpoint", level = "debug", skip(self, frame))]
    pub fn send(&self, frame: EthernetFrame) {
        tracing::debug!("EndPoint send: frame={:?}", frame);
        cast!(self.addr, EndPointMsg::Send(frame)).unwrap()
    }

    #[tracing::instrument(target = "endpoint", level = "debug", skip(self, frame))]
    pub fn write(&self, frame: EthernetFrame) {
        tracing::debug!("EndPoint write: frame={:?}", frame);
        let _ = cast!(self.addr, EndPointMsg::Write(frame));
    }

    #[tracing::instrument(target = "endpoint", level = "trace", skip(self))]
    pub fn actor_ref(&self) -> &ActorRef<EndPointMsg> {
        &self.addr
    }

    #[tracing::instrument(target = "endpoint", level = "info", skip(core, on_receive))]
    pub async fn spawn(
        core: Core,
        uuid: Uuid,
        on_receive: DerivedActorRef<OnReceiveRaw>,
    ) -> ActorRef<EndPointMsg> {
        tracing::info!("Spawning EndPoint: uuid={:?}", uuid);
        let (addr, _) = EndPointActor::spawn(None, EndPointActor, (uuid, core, on_receive))
            .await
            .unwrap();
        tracing::info!("EndPoint spawned: addr={:?}", addr);
        addr
    }
}

#[derive(Debug)]
pub struct EndPointActorState {
    uuid: Uuid,
    core: Core,
    on_receive: DerivedActorRef<OnReceiveRaw>,
}

#[derive(Debug)]
pub struct EndPointActor;

#[ractor::async_trait]
impl Actor for EndPointActor {
    type Msg = EndPointMsg;
    type State = EndPointActorState;
    type Arguments = (Uuid, Core, DerivedActorRef<OnReceiveRaw>);

    #[tracing::instrument(target = "endpoint_actor", level = "info", skip_all)]
    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        (uuid, core, on_receive): Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        tracing::info!("EndPointActor pre_start: uuid={:?}", uuid);
        Ok(EndPointActorState {
            uuid,
            core,
            on_receive,
        })
    }

    #[tracing::instrument(target = "endpoint_actor", level = "debug", skip_all)]
    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        tracing::trace!("EndPointActor handle: msg={:?}", msg);
        match msg {
            EndPointMsg::GetUuid(reply) => {
                tracing::trace!("EndPointActor: GetUuid -> {:?}", state.uuid);
                let _ = reply.send(state.uuid);
            },
            EndPointMsg::Send(frame) => {
                tracing::debug!("EndPointActor: Send frame={:?}", frame);
                state.core.send(state.uuid, frame);
            },
            EndPointMsg::Write(frame) => {
                tracing::debug!("EndPointActor: Write frame={:?}", frame);
                let _ = state.on_receive.cast(OnReceiveRaw {
                    dst: state.uuid,
                    payload: frame,
                });
            },
        }
        Ok(())
    }
}
