use std::collections::{HashMap, VecDeque};
use std::net::Ipv4Addr;
use std::usize;
use tracing::instrument;

use crate::{
    ArpOperation, ArpPacket, Core, EndPoint, EthernetCard, EthernetFrame, EthernetFrameType,
    IPv4Packet, IPv4PacketType, OnReceive, OnReceiveRaw, Uuid, handle_arp, mac_rnd,
};
use futures::future::join_all;
use ipnet::Ipv4Subnets;
use macaddr::MacAddr6;
use ractor::concurrency::mpsc_unbounded;
use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort, call, cast};
use rand::Rng;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc::{Receiver, UnboundedReceiver, UnboundedSender};

pub struct PortConfig {
    subnet: Ipv4Addr,
    ip: Ipv4Addr,
    mac: MacAddr6,
}

pub struct Port {
    subnet: Ipv4Addr,
    nic: NetworkInterfaceCard,
    ip: Ipv4Addr,
    uuid: Uuid,
}

pub struct L3Sw {
    addr: ActorRef<L3SwMsg>,
}

impl L3Sw {
    pub async fn port(&self, idx: PortIdx) -> Option<NetworkInterfaceCard> {
        call!(self.addr, L3SwMsg::GetPort, idx).unwrap()
    }
}

pub struct L3SwBuilder {
    core: Core,
    ports_config: Vec<PortConfig>,
}

impl L3SwBuilder {
    #[instrument(skip(core))]
    pub fn new(core: Core) -> Self {
        Self {
            core,
            ports_config: vec![],
        }
    }

    #[instrument(skip(self))]
    pub fn port(mut self, ip: Ipv4Addr, subnet: Ipv4Addr) -> Self {
        self.port_with_mac(ip, subnet, mac_rnd())
    }

    #[instrument(skip(self))]
    pub fn port_with_mac(mut self, ip: Ipv4Addr, subnet: Ipv4Addr, mac: MacAddr6) -> Self {
        self.ports_config.push(PortConfig { subnet, ip, mac });
        self
    }

    #[instrument(skip(self))]
    pub async fn spawn(self) -> L3Sw {
        let (addr, _) = L3SwActor::spawn(None, L3SwActor, (self.core, self.ports_config))
            .await
            .unwrap();
        tracing::info!("L3SwBuilder spawn called");
        L3Sw { addr }
    }
}

impl L3Sw {}

type PortIdx = usize;

#[derive(Debug)]
pub enum Route {
    Directly {
        dst: (Ipv4Addr, Ipv4Addr),
        r#if: PortIdx,
    },
}

pub struct RoutingTable {
    routes: Vec<Route>,
}

impl RoutingTable {
    pub fn r#match(&self, dst_ip: Ipv4Addr) -> Option<&Route> {
        self.routes.iter().find_map(|r| match r {
            Route::Directly { dst, r#if } => {
                let network = dst.0 & dst.1;
                if network == dst_ip & dst.1 {
                    Some(r)
                } else {
                    None
                }
            },
        })
    }
}

pub enum L3SwMsg {
    OnReceive(OnReceive),
    GetPort(PortIdx, RpcReplyPort<Option<NetworkInterfaceCard>>),
}

impl From<OnReceive> for L3SwMsg {
    fn from(value: OnReceive) -> Self {
        L3SwMsg::OnReceive(value)
    }
}

impl TryFrom<L3SwMsg> for OnReceive {
    type Error = String;

    fn try_from(value: L3SwMsg) -> Result<Self, Self::Error> {
        match value {
            L3SwMsg::OnReceive(value) => Ok(value),
            _ => Err("invalid try form".to_string()),
        }
    }
}

pub struct L3SwActorState {
    ports: Vec<Port>,
    rtb: RoutingTable,
}

pub struct L3SwActor;

#[ractor::async_trait]
impl Actor for L3SwActor {
    type Msg = L3SwMsg;
    type State = L3SwActorState;
    type Arguments = (Core, Vec<PortConfig>);

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        (core, ports_config): Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let ports = ports_config.iter().map(async |pc| {
            let nic = NetworkInterfaceCardBuilder::new()
                .core(core.clone())
                .ip(pc.ip)
                .mac(pc.mac)
                .promiscuous(true)
                .spawn()
                .await;
            let uuid = nic.uuid().await;

            Port {
                subnet: pc.subnet,
                nic,
                uuid,
                ip: pc.ip,
            }
        });

        let ports = join_all(ports).await;

        let routes = ports
            .iter()
            .enumerate()
            .map(|(r#if, port)| Route::Directly {
                dst: (port.ip, port.subnet),
                r#if,
            })
            .collect();
        let rtb = RoutingTable { routes };

        Ok(L3SwActorState { ports, rtb })
    }

    #[instrument(skip(self, _myself, msg, state))]
    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            L3SwMsg::OnReceive(onr) => match onr.payload.payload {
                EthernetFrameType::IPv4(packet) => {
                    tracing::info!(src=?packet.src, dst=?packet.dst, "IPv4 packet received");
                    if let Some(route) = state.rtb.r#match(packet.dst) {
                        tracing::info!(route=?route, "route found");
                        match route {
                            Route::Directly { dst, r#if } => {
                                let ifport = state.ports.get_mut(*r#if).expect("unreachable!");
                                tracing::info!(out_port=?r#if, "forwarding packet");
                                ifport.nic.send(packet.dst, packet.payload);
                            },
                        }
                    } else {
                        tracing::info!(dst=?packet.dst, "no route found");
                    }
                },
                _ => {
                    tracing::info!("non-IPv4 packet received");
                },
            },
            L3SwMsg::GetPort(idx, reply) => {
                if let Some(port) = state.ports.get(idx) {
                    reply.send(Some(port.nic.clone())).unwrap();
                } else {
                    reply.send(None).unwrap();
                }
            },
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct DefaultGateway {
    ip: Ipv4Addr,
    subnet: Ipv4Addr,
}

impl DefaultGateway {
    pub fn new(ip: Ipv4Addr, subnet: Ipv4Addr) -> Self {
        DefaultGateway { ip, subnet }
    }

    pub fn network(&self) -> Ipv4Addr {
        self.ip & self.subnet
    }

    pub fn is_belong(&self, ip: Ipv4Addr) -> bool {
        self.network() == ip & self.subnet
    }
}

#[derive(Clone)]
pub struct NetworkInterfaceCard {
    addr: ActorRef<NetworkInterfaceCardMsg>,
    ip: Ipv4Addr,
    mac: MacAddr6,
    promiscuous: bool,
    dgw: Option<DefaultGateway>,
}

pub struct NetworkInterfaceCardBuilder {
    core: Option<Core>,
    ip: Option<Ipv4Addr>,
    mac: Option<MacAddr6>,
    tag: Option<String>,
    promiscuous: bool,
    dgw: Option<DefaultGateway>,
}

impl NetworkInterfaceCardBuilder {
    pub fn new() -> Self {
        Self {
            core: None,
            ip: None,
            mac: None,
            tag: None,
            promiscuous: false,
            dgw: None,
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
    pub fn promiscuous(mut self, enable: bool) -> Self {
        self.promiscuous = enable;
        self
    }
    pub fn default_gateway(mut self, dgw: DefaultGateway) -> Self {
        self.dgw = Some(dgw);
        self
    }
    pub async fn spawn(self) -> NetworkInterfaceCard {
        let core = self.core.expect("core is required");
        let ip = self.ip.expect("ip is required");
        let mac = self.mac.unwrap_or_else(|| mac_rnd());
        let tag = self.tag.clone();
        let promiscuous = self.promiscuous;
        // let dgw = self.dgw.expect("default gataway is needed");

        let (addr, _) = NetworkInterfaceCardActor::spawn(
            tag,
            NetworkInterfaceCardActor,
            (core, ip, mac, promiscuous, self.dgw.clone()),
        )
        .await
        .unwrap();
        NetworkInterfaceCard {
            addr,
            ip,
            mac,
            promiscuous,
            dgw: self.dgw,
        }
    }
}

impl NetworkInterfaceCard {
    pub fn send(&self, dst: Ipv4Addr, payload: IPv4PacketType) {
        cast!(self.addr, NetworkInterfaceCardMsg::Send(dst, payload)).unwrap();
    }

    pub async fn recv_blocking(&self) -> IPv4Packet {
        let pkt = call!(self.addr, NetworkInterfaceCardMsg::RecvBlocking).unwrap();
        pkt
    }

    pub async fn uuid(&self) -> Uuid {
        let uuid = call!(self.addr, NetworkInterfaceCardMsg::GetUuid).unwrap();
        uuid
    }

    pub fn actor_ref(&self) -> &ActorRef<NetworkInterfaceCardMsg> {
        &self.addr
    }

    pub fn promiscuous(&self) -> bool {
        self.promiscuous
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
    dgw: Option<DefaultGateway>,
    promiscuous: bool,
    arp_cache: HashMap<Ipv4Addr, MacAddr6>,
    tx_pendings: HashMap<Ipv4Addr, Vec<IPv4PacketType>>,
    rx_buffer_r: Arc<Mutex<UnboundedReceiver<IPv4Packet>>>,
    rx_buffer_s: UnboundedSender<IPv4Packet>,
}

#[ractor::async_trait]
impl Actor for NetworkInterfaceCardActor {
    type Msg = NetworkInterfaceCardMsg;
    type State = NetworkInterfaceCardActorState;
    type Arguments = (Core, Ipv4Addr, MacAddr6, bool, Option<DefaultGateway>);

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        (core, ip, mac, promiscuous, dgw): Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let me = myself.get_derived();
        let eth = EthernetCard::spawn(core, mac, promiscuous, me.clone()).await;
        let (rx_buffer_s, rx_buffer_r) = mpsc_unbounded();
        let r = Ok(NetworkInterfaceCardActorState {
            eth,
            ip,
            mac,
            dgw,
            promiscuous,
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
            NetworkInterfaceCardMsg::Send(mut dst, payload) => {
                if let Some(dgw) = &state.dgw {
                    if !dgw.is_belong(dst) {
                        dst = dgw.ip;
                    }
                }

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
                EthernetFrameType::IPv4(packet) => {
                    if packet.dst == state.ip || packet.dst.is_broadcast() || state.promiscuous {
                        state.rx_buffer_s.send(packet).unwrap()
                    }
                },
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
