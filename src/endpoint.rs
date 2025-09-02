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
    #[instrument(skip(addr))]
    pub fn new(addr: ActorRef<EndPointMsg>) -> Self {
        EndPoint { addr }
    }

    #[instrument(skip(self))]
    pub async fn uuid(&self) -> Uuid {
        let uuid = call!(self.addr, EndPointMsg::GetUuid).unwrap();
        tracing::info!(uuid = ?uuid, "uuid called");
        uuid
    }

    #[instrument(skip(self, frame))]
    pub fn send(&self, frame: EthernetFrame) {
        tracing::info!("send called");
        cast!(self.addr, EndPointMsg::Send(frame)).unwrap()
    }

    #[instrument(skip(self, frame))]
    pub fn write(&self, frame: EthernetFrame) {
        tracing::info!("write called");
        let _ = cast!(self.addr, EndPointMsg::Write(frame));
    }

    #[instrument(skip(self))]
    pub fn actor_ref(&self) -> &ActorRef<EndPointMsg> {
        &self.addr
    }

    #[instrument(skip(core, on_receive))]
    pub async fn spawn(
        core: Core,
        uuid: Uuid,
        on_receive: DerivedActorRef<OnReceiveRaw>,
    ) -> ActorRef<EndPointMsg> {
        let (addr, _) = EndPointActor::spawn(None, EndPointActor, (uuid, core, on_receive))
            .await
            .unwrap();
        tracing::info!(uuid = ?uuid, "spawn called");
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

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        (uuid, core, on_receive): Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(EndPointActorState {
            uuid,
            core,
            on_receive,
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
            EndPointMsg::GetUuid(_) => {
                tracing::info!(uuid = ?state.uuid, "handle GetUuid");
            },
            EndPointMsg::Send(_) => {
                tracing::info!(uuid = ?state.uuid, "handle Send");
            },
            EndPointMsg::Write(_) => {
                tracing::info!(uuid = ?state.uuid, "handle Write");
            },
        }
        match msg {
            EndPointMsg::GetUuid(reply) => {
                let _ = reply.send(state.uuid);
            },
            EndPointMsg::Send(frame) => {
                state.core.send(state.uuid, frame);
            },
            EndPointMsg::Write(frame) => {
                let _ = state.on_receive.cast(OnReceiveRaw {
                    dst: state.uuid,
                    payload: frame,
                });
            },
        }
        Ok(())
    }
}
