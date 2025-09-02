use std::collections::HashMap;

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
    pub async fn spawn(
        core: Core,
        mac: MacAddr6,
        is_promiscuous: bool,
        on_receive: DerivedActorRef<OnReceive>,
    ) -> Self {
        info!(
            "Spawning EthernetCard: mac={:?} promisc={}",
            mac, is_promiscuous
        );
        let (addr, _) = EthernetCardActor::spawn(
            None,
            EthernetCardActor,
            (core, mac, is_promiscuous, on_receive),
        )
        .await
        .unwrap();
        info!("EthernetCard spawned: addr={:?}", addr);

        Self { addr }
    }

    pub async fn send(&self, dst: MacAddr6, payload: EthernetFrameType) {
        // debug!("EthernetCard send: dst={:?} payload={:?}", dst, payload);
        let _ = cast!(self.addr, EthernetMsg::Send(dst, payload));
    }

    pub async fn uuid(&self) -> Uuid {
        let uuid = call!(self.addr, EthernetMsg::GetUuid).unwrap();
        trace!("EthernetCard uuid: {:?}", uuid);
        uuid
    }

    pub fn write(&self, onr: OnReceiveRaw) {
        debug!("EthernetCard write: onr={:?}", onr);
        let _ = cast!(self.addr, EthernetMsg::OnReceive(onr));
    }

    pub fn filter() {
        // tracing_subscriber::fmt::layer().with_filter(filter::filter_fn(|metadata| {
        //     metadata.target() == "ethernetcard"
        // }))
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
        // ...existing code...
        let on_receive_raw: DerivedActorRef<OnReceiveRaw> = myself.get_derived();
        let ep = core.create_ep(on_receive_raw).await;
        // ...existing code...
        Ok(EthernetCardActorState {
            mac,
            ep,
            is_promiscuous,
            on_receive,
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        // ...existing code...
        match msg {
            EthernetMsg::Send(dst, payload) => {
                // ...existing code...
                let ef = EthernetFrame::new(state.mac, dst, payload);
                state.ep.send(ef);
            },
            EthernetMsg::GetUuid(reply) => {
                let uuid = state.ep.uuid().await;
                // ...existing code...
                let _ = reply.send(uuid);
            },
            EthernetMsg::OnReceive(onr) => {
                // ...existing code...
                let r = cast!(
                    state.on_receive,
                    OnReceive {
                        payload: onr.payload,
                        dst: onr.dst,
                    }
                );
                // ...existing code...
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
