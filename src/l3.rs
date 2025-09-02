use std::collections::{HashMap, VecDeque};
use std::net::Ipv4Addr;
use std::usize;

use crate::{
    ArpOperation, ArpPacket, Core, EthernetCard, EthernetFrame, EthernetFrameType, IPv4Packet,
    IPv4PacketType, OnReceive, Uuid, handle_arp,
};
use macaddr::MacAddr6;
use ractor::concurrency::mpsc_unbounded;
use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort, call, cast};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc::{Receiver, UnboundedReceiver, UnboundedSender};

pub struct NetworkInterfaceCard {
    addr: ActorRef<NetworkInterfaceCardMsg>,
    ip: Ipv4Addr,
    mac: MacAddr6,
}

pub struct NetworkInterfaceCardBuilder {
    core: Option<Core>,
    ip: Option<Ipv4Addr>,
    mac: Option<MacAddr6>,
    tag: Option<String>,
}

impl NetworkInterfaceCardBuilder {
    pub fn new() -> Self {
        Self {
            core: None,
            ip: None,
            mac: None,
            tag: None,
        }
    }
    pub fn core(mut self, core: Core) -> Self {
        self.core = Some(core);
        self
    }
    pub fn ip(mut self, ip: Ipv4Addr) -> Self {
        self.ip = Some(ip);
        self
    }
    pub fn mac(mut self, mac: MacAddr6) -> Self {
        self.mac = Some(mac);
        self
    }
    pub fn tag<S: Into<String>>(mut self, tag: S) -> Self {
        self.tag = Some(tag.into());
        self
    }
    pub async fn spawn(self) -> NetworkInterfaceCard {
        let core = self.core.expect("core is required");
        let ip = self.ip.expect("ip is required");
        let mac = self.mac.expect("mac is required");
        let tag = self.tag.clone();

        let (addr, _) =
            NetworkInterfaceCardActor::spawn(tag, NetworkInterfaceCardActor, (core, ip, mac))
                .await
                .unwrap();
        // ...existing code...
        NetworkInterfaceCard { addr, ip, mac }
    }
}

impl NetworkInterfaceCard {
    #[deprecated(note = "Use NetworkInterfaceCardBuilder instead")]
    pub async fn spawn(core: Core, ip: Ipv4Addr, mac: MacAddr6) -> Self {
        NetworkInterfaceCardBuilder::new()
            .core(core)
            .ip(ip)
            .mac(mac)
            .spawn()
            .await
    }

    pub fn send(&self, dst: Ipv4Addr, payload: IPv4PacketType) {
        // ...existing code...
        cast!(self.addr, NetworkInterfaceCardMsg::Send(dst, payload)).unwrap();
    }

    pub async fn recv_blocking(&self) -> IPv4Packet {
        // ...existing code...
        let pkt = call!(self.addr, NetworkInterfaceCardMsg::RecvBlocking).unwrap();
        // ...existing code...
        pkt
    }

    pub async fn uuid(&self) -> Uuid {
        let uuid = call!(self.addr, NetworkInterfaceCardMsg::GetUuid).unwrap();
        // ...existing code...
        uuid
    }

    pub fn actor_ref(&self) -> &ActorRef<NetworkInterfaceCardMsg> {
        &self.addr
    }
}

#[derive(Debug)]
pub enum NetworkInterfaceCardMsg {
    Send(Ipv4Addr, IPv4PacketType),
    OnReceive(EthernetFrame),
    GetUuid(RpcReplyPort<Uuid>),
    RecvBlocking(RpcReplyPort<IPv4Packet>),
}

impl From<OnReceive> for NetworkInterfaceCardMsg {
    fn from(value: OnReceive) -> Self {
        NetworkInterfaceCardMsg::OnReceive(value.payload)
    }
}

impl TryFrom<NetworkInterfaceCardMsg> for OnReceive {
    type Error = String;

    fn try_from(value: NetworkInterfaceCardMsg) -> Result<Self, Self::Error> {
        match value {
            NetworkInterfaceCardMsg::OnReceive(ef) => Ok(OnReceive {
                payload: ef,
                dst: usize::MAX,
            }),
            _ => Err("invalid try form".to_string()),
        }
    }
}

pub struct NetworkInterfaceCardActor;

pub struct NetworkInterfaceCardActorState {
    eth: EthernetCard,
    ip: Ipv4Addr,
    mac: MacAddr6,
    arp_cache: HashMap<Ipv4Addr, MacAddr6>,
    tx_pendings: HashMap<Ipv4Addr, Vec<IPv4PacketType>>,
    rx_buffer_r: Arc<Mutex<UnboundedReceiver<IPv4Packet>>>,
    rx_buffer_s: UnboundedSender<IPv4Packet>,
}

#[ractor::async_trait]
impl Actor for NetworkInterfaceCardActor {
    type Msg = NetworkInterfaceCardMsg;
    type State = NetworkInterfaceCardActorState;
    type Arguments = (Core, Ipv4Addr, MacAddr6);

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        (core, ip, mac): Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let me = myself.get_derived();
        let eth = EthernetCard::spawn(core, mac, false, me.clone()).await;
        let (rx_buffer_s, rx_buffer_r) = mpsc_unbounded();
        let r = Ok(NetworkInterfaceCardActorState {
            eth,
            ip,
            mac,
            arp_cache: HashMap::new(),
            tx_pendings: HashMap::new(),
            rx_buffer_r: Arc::new(Mutex::new(rx_buffer_r)),
            rx_buffer_s,
        });
        r
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            NetworkInterfaceCardMsg::Send(dst, payload) => {
                if let Some(mac) = state.arp_cache.get(&dst) {
                    state
                        .eth
                        .send(
                            *mac,
                            EthernetFrameType::IPv4(IPv4Packet::new(state.ip, dst, payload)),
                        )
                        .await;
                } else {
                    state
                        .tx_pendings
                        .entry(dst)
                        .or_default()
                        .push(payload.clone());
                    let req = ArpPacket::mk_request(dst, state.ip, state.mac);
                    state
                        .eth
                        .send(MacAddr6::broadcast(), EthernetFrameType::Arp(req))
                        .await;
                }
            },
            NetworkInterfaceCardMsg::OnReceive(ef) => match ef.payload {
                EthernetFrameType::IPv4(packet) => state.rx_buffer_s.send(packet).unwrap(),
                EthernetFrameType::Arp(arp) => self.handle_arp(state, arp).await,
                _ => {},
            },
            NetworkInterfaceCardMsg::RecvBlocking(reply) => {
                let rx = state.rx_buffer_r.clone();
                tokio::spawn(async move {
                    let mut guard = rx.lock().await;
                    if let Some(packet) = guard.recv().await {
                        let _ = reply.send(packet);
                    }
                });
            },
            NetworkInterfaceCardMsg::GetUuid(reply) => {
                let uuid = state.eth.uuid().await;
                let _ = reply.send(uuid);
            },
        }
        Ok(())
    }
}

impl NetworkInterfaceCardActor {
    async fn handle_arp(&self, state: &mut NetworkInterfaceCardActorState, arp: ArpPacket) {
        let action = handle_arp(
            &arp,
            state.ip,
            state.mac,
            state.arp_cache.clone(),
            state.tx_pendings.clone(),
        );

        state.arp_cache = action.updated_arp_cache;
        state.tx_pendings = action.updated_tx_pendings;

        for (dst_mac, frame_type) in action.send_frames {
            state.eth.send(dst_mac, frame_type).await;
        }
    }
}
