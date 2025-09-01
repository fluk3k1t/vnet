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
use tracing::debug;

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
    #[tracing::instrument(target = "l3_builder", level = "info")]
    pub fn new() -> Self {
        Self {
            core: None,
            ip: None,
            mac: None,
            tag: None,
        }
    }
    #[tracing::instrument(target = "l3_builder", level = "debug", skip(self, core))]
    pub fn core(mut self, core: Core) -> Self {
        self.core = Some(core);
        self
    }
    #[tracing::instrument(target = "l3_builder", level = "debug", skip(self))]
    pub fn ip(mut self, ip: Ipv4Addr) -> Self {
        self.ip = Some(ip);
        self
    }
    #[tracing::instrument(target = "l3_builder", level = "debug", skip(self))]
    pub fn mac(mut self, mac: MacAddr6) -> Self {
        self.mac = Some(mac);
        self
    }
    #[tracing::instrument(target = "l3_builder", level = "debug", skip(self, tag))]
    pub fn tag<S: Into<String>>(mut self, tag: S) -> Self {
        self.tag = Some(tag.into());
        self
    }
    #[tracing::instrument(target = "l3_builder", level = "info", skip(self))]
    pub async fn spawn(self) -> NetworkInterfaceCard {
        let core = self.core.expect("core is required");
        let ip = self.ip.expect("ip is required");
        let mac = self.mac.expect("mac is required");
        let tag = self.tag.clone();
        tracing::info!(
            "Spawning NetworkInterfaceCard with tag={:?}, ip={:?}, mac={:?}",
            tag,
            ip,
            mac
        );
        let (addr, _) =
            NetworkInterfaceCardActor::spawn(tag, NetworkInterfaceCardActor, (core, ip, mac))
                .await
                .unwrap();
        tracing::info!("NetworkInterfaceCard spawned: addr={:?}", addr);
        NetworkInterfaceCard { addr, ip, mac }
    }
}

impl NetworkInterfaceCard {
    #[deprecated(note = "Use NetworkInterfaceCardBuilder instead")]
    #[tracing::instrument(target = "l3", level = "info")]
    pub async fn spawn(core: Core, ip: Ipv4Addr, mac: MacAddr6) -> Self {
        NetworkInterfaceCardBuilder::new()
            .core(core)
            .ip(ip)
            .mac(mac)
            .spawn()
            .await
    }

    #[tracing::instrument(target = "l3", level = "debug", skip(self, payload))]
    pub fn send(&self, dst: Ipv4Addr, payload: IPv4PacketType) {
        tracing::debug!("NIC({:?}) send to {:?}: {:?}", self.mac, dst, payload);
        cast!(self.addr, NetworkInterfaceCardMsg::Send(dst, payload)).unwrap();
    }

    #[tracing::instrument(target = "l3", level = "debug", skip(self))]
    pub async fn recv_blocking(&self) -> IPv4Packet {
        tracing::debug!("NIC({:?}) waiting for packet", self.mac);
        let pkt = call!(self.addr, NetworkInterfaceCardMsg::RecvBlocking).unwrap();
        tracing::debug!("NIC({:?}) received packet: {:?}", self.mac, pkt);
        pkt
    }

    #[tracing::instrument(target = "l3", level = "trace", skip(self))]
    pub async fn uuid(&self) -> Uuid {
        let uuid = call!(self.addr, NetworkInterfaceCardMsg::GetUuid).unwrap();
        tracing::trace!("NIC({:?}) uuid: {:?}", self.mac, uuid);
        uuid
    }

    #[tracing::instrument(target = "l3", level = "trace", skip(self))]
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
        debug!("from {:?}", value);
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
        tracing::info!(
            "NetworkInterfaceCardActor pre_start: ip={:?}, mac={:?}",
            ip,
            mac
        );
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
        tracing::info!("NetworkInterfaceCardActor pre_start done");
        r
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        tracing::trace!("NIC actor handle: msg={:?}", msg);
        match msg {
            NetworkInterfaceCardMsg::Send(dst, payload) => {
                tracing::debug!("NIC actor: Send to {:?} payload={:?}", dst, payload);
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
                    tracing::debug!("NIC actor: ARP request for {:?}", dst);
                    state
                        .eth
                        .send(MacAddr6::broadcast(), EthernetFrameType::Arp(req))
                        .await;
                }
            },
            NetworkInterfaceCardMsg::OnReceive(ef) => {
                tracing::debug!("NIC actor: OnReceive frame={:?}", ef);
                match ef.payload {
                    EthernetFrameType::IPv4(packet) => {
                        tracing::debug!("NIC actor: Received IPv4 packet: {:?}", packet);
                        state.rx_buffer_s.send(packet).unwrap()
                    },
                    EthernetFrameType::Arp(arp) => {
                        tracing::debug!("NIC actor: Received ARP packet: {:?}", arp);
                        self.handle_arp(state, arp).await
                    },
                    _ => {},
                }
            },
            NetworkInterfaceCardMsg::RecvBlocking(reply) => {
                tracing::debug!("NIC actor: RecvBlocking");
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
                tracing::trace!("NIC actor: GetUuid -> {:?}", uuid);
                let _ = reply.send(uuid);
            },
        }
        Ok(())
    }
}

impl NetworkInterfaceCardActor {
    async fn handle_arp(&self, state: &mut NetworkInterfaceCardActorState, arp: ArpPacket) {
        tracing::debug!("NIC actor: handle_arp called: {:?}", arp);
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
            tracing::debug!(
                "NIC actor: handle_arp sending frame to {:?}: {:?}",
                dst_mac,
                frame_type
            );
            state.eth.send(dst_mac, frame_type).await;
        }
    }
}
