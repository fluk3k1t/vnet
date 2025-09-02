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
    pub fn new(addr: ActorRef<EndPointMsg>) -> Self {
        EndPoint { addr }
    }

    #[instrument]
    pub async fn uuid(&self) -> Uuid {
        let uuid = call!(self.addr, EndPointMsg::GetUuid).unwrap();

        uuid
    }

    pub fn send(&self, frame: EthernetFrame) {
        cast!(self.addr, EndPointMsg::Send(frame)).unwrap()
    }

    pub fn write(&self, frame: EthernetFrame) {
        let _ = cast!(self.addr, EndPointMsg::Write(frame));
    }

    pub fn actor_ref(&self) -> &ActorRef<EndPointMsg> {
        &self.addr
    }

    pub async fn spawn(
        core: Core,
        uuid: Uuid,
        on_receive: DerivedActorRef<OnReceiveRaw>,
    ) -> ActorRef<EndPointMsg> {
        let (addr, _) = EndPointActor::spawn(None, EndPointActor, (uuid, core, on_receive))
            .await
            .unwrap();
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

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
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
