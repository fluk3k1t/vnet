use std::collections::HashMap;
use tracing::instrument;

use crate::{Core, EndPoint, EthernetFrame, EthernetFrameType, OnReceiveRaw, Uuid};
use futures::{StreamExt, future::join_all, stream::repeat_with};
use macaddr::MacAddr6;
use ractor::{Actor, ActorProcessingErr, ActorRef, DerivedActorRef, RpcReplyPort, call, cast};
use tracing::{Level, debug, info, trace};
use tracing_subscriber::{
    Layer,
    filter::{self, Targets},
};

pub struct EthernetCard {
    addr: ActorRef<EthernetMsg>,
}

#[derive(Debug)]
pub struct OnReceive {
    pub payload: EthernetFrame,
    pub dst: Uuid,
}

impl EthernetCard {
    #[instrument(skip(core, on_receive))]
    pub async fn spawn(
        core: Core,
        mac: MacAddr6,
        is_promiscuous: bool,
        on_receive: DerivedActorRef<OnReceive>,
    ) -> Self {
        let (addr, _) = EthernetCardActor::spawn(
            None,
            EthernetCardActor,
            (core, mac, is_promiscuous, on_receive),
        )
        .await
        .unwrap();

        Self { addr }
    }

    #[instrument(skip(self, payload))]
    pub async fn send(&self, dst: MacAddr6, payload: EthernetFrameType) {
        tracing::info!(?dst, "send called");
        let _ = cast!(self.addr, EthernetMsg::Send(dst, payload));
    }

    #[instrument(skip(self))]
    pub async fn uuid(&self) -> Uuid {
        let uuid = call!(self.addr, EthernetMsg::GetUuid).unwrap();
        uuid
    }

    #[instrument(skip(self, onr))]
    pub fn write(&self, onr: OnReceiveRaw) {
        tracing::info!(?onr, "write called");
        debug!("EthernetCard write: onr={:?}", onr);
        let _ = cast!(self.addr, EthernetMsg::OnReceive(onr));
    }
}

struct EthernetCardActor;

#[derive(Debug)]
pub enum EthernetMsg {
    Send(MacAddr6, EthernetFrameType),
    GetUuid(RpcReplyPort<Uuid>),
    OnReceive(OnReceiveRaw),
}

impl From<OnReceiveRaw> for EthernetMsg {
    fn from(value: OnReceiveRaw) -> Self {
        EthernetMsg::OnReceive(value)
    }
}

impl TryFrom<EthernetMsg> for OnReceiveRaw {
    type Error = String;

    fn try_from(value: EthernetMsg) -> Result<Self, Self::Error> {
        match value {
            EthernetMsg::OnReceive(value) => Ok(value),
            _ => Err("invalid try form".to_string()),
        }
    }
}

#[ractor::async_trait]
impl Actor for EthernetCardActor {
    type Msg = EthernetMsg;
    type State = EthernetCardActorState;
    type Arguments = (Core, MacAddr6, bool, DerivedActorRef<OnReceive>);

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        (core, mac, is_promiscuous, on_receive): Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let on_receive_raw: DerivedActorRef<OnReceiveRaw> = myself.get_derived();
        let ep = core.create_ep(on_receive_raw).await;

        Ok(EthernetCardActorState {
            mac,
            ep,
            is_promiscuous,
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
            EthernetMsg::Send(dst, _) => {
                tracing::info!(?dst, "handle Send");
            },
            EthernetMsg::GetUuid(_) => {
                tracing::info!("handle GetUuid");
            },
            EthernetMsg::OnReceive(onr) => {
                tracing::info!(?onr, "handle OnReceive");
            },
        }
        match msg {
            EthernetMsg::Send(dst, payload) => {
                let ef = EthernetFrame::new(state.mac, dst, payload);
                state.ep.send(ef);
            },
            EthernetMsg::GetUuid(reply) => {
                let uuid = state.ep.uuid().await;
                let _ = reply.send(uuid);
            },
            EthernetMsg::OnReceive(onr) => {
                if onr.payload.dst == state.mac
                    || onr.payload.dst.is_broadcast()
                    || state.is_promiscuous
                {
                    cast!(
                        state.on_receive,
                        OnReceive {
                            payload: onr.payload,
                            dst: onr.dst,
                        }
                    )
                    .unwrap();
                }
            },
        }
        Ok(())
    }
}

pub struct EthernetCardActorState {
    mac: MacAddr6,
    ep: EndPoint,
    is_promiscuous: bool,
    on_receive: DerivedActorRef<OnReceive>,
}

#[derive(Clone)]
pub struct L2Sw {
    addr: ActorRef<L2SwMsg>,
}

impl L2Sw {
    pub async fn spawn(core: Core, n_ports: usize) -> Self {
        let (addr, _) = L2SwActor::spawn(None, L2SwActor, (core, n_ports))
            .await
            .unwrap();
        L2Sw { addr }
    }

    pub async fn port(&self, idx: usize) -> Option<EndPoint> {
        call!(self.addr, L2SwMsg::GetPort, idx).unwrap()
    }
}

type PortIdx = usize;

#[derive(Debug)]
pub enum L2SwMsg {
    GetPort(PortIdx, RpcReplyPort<Option<EndPoint>>),
    OnReceive(OnReceiveRaw),
}

impl From<OnReceiveRaw> for L2SwMsg {
    fn from(value: OnReceiveRaw) -> Self {
        L2SwMsg::OnReceive(value)
    }
}

impl TryFrom<L2SwMsg> for OnReceiveRaw {
    type Error = String;

    fn try_from(value: L2SwMsg) -> Result<Self, Self::Error> {
        match value {
            L2SwMsg::OnReceive(value) => Ok(value),
            _ => Err("invalid try form".to_string()),
        }
    }
}

pub struct L2SwActorState {
    ports: Vec<(Uuid, EndPoint)>,
}

pub struct L2SwActor;

#[ractor::async_trait]
impl Actor for L2SwActor {
    type Msg = L2SwMsg;
    type State = L2SwActorState;
    type Arguments = (Core, usize);

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        (core, n_ports): Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let ports = repeat_with(|| async {
            let ep = core.create_ep(myself.get_derived()).await;
            (ep.uuid().await, ep)
        })
        .take(n_ports)
        .collect::<Vec<_>>()
        .await;

        let ports = join_all(ports).await;

        Ok(L2SwActorState { ports })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            L2SwMsg::GetPort(port_idx, reply) => {
                let port = state.ports.get(port_idx).map(|(a, b)| b.clone());
                let _ = reply.send(port);
            },
            L2SwMsg::OnReceive(onr) => {
                for (uuid, port) in state.ports.iter_mut() {
                    if *uuid != onr.dst {
                        port.send(onr.payload.clone());
                    }
                }
            },
        }
        Ok(())
    }
}
