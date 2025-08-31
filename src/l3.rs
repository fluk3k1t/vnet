use std::{
    collections::{HashMap, VecDeque},
    iter::repeat_with,
    net::Ipv4Addr,
    ops::Index,
    sync::Arc,
};

use actix::prelude::*;
use futures::future::join_all;
use macaddr::MacAddr6;
use tokio::sync::Mutex;
use tracing::{debug, instrument, trace};

use crate::{
    ArpOperation, ArpPacket, Connectable, Core, EndPoint, EthernetCard, EthernetFrame,
    EthernetFrameType, IPv4Packet, IPv4PacketType, OnReceive, Uuid,
};

#[derive(Clone)]
pub struct NetworkCard {
    addr: Addr<NetworkCardRaw>,
    ip: Ipv4Addr,
    mac: MacAddr6,
}

pub struct NetworkCardBuilder {
    ip: Ipv4Addr,
    core: Core,
    mac: MacAddr6,
    is_promiscuous: bool,
}

impl NetworkCard {
    pub fn new(core: Core, ip: Ipv4Addr, mac: MacAddr6) -> NetworkCardBuilder {
        NetworkCardBuilder {
            core,
            ip,
            mac,
            is_promiscuous: false,
        }
    }

    #[instrument(skip(self))]
    pub fn send(&self, dst: MacAddr6, payload: EthernetFrameType) {
        self.addr.do_send(NetworkCardRawSend { dst, payload });
    }

    #[instrument(skip(self))]
    pub async fn recv(&self) -> Option<EthernetFrame> {
        self.addr.send(NetworkCardRawRecv).await.unwrap()
    }

    pub async fn uuid(&self) -> Uuid {
        self.addr.send(NetworkCardRawGetUuid).await.unwrap()
    }
}

impl NetworkCardBuilder {
    pub fn promiscuous(mut self, is_promiscuous: bool) -> Self {
        self.is_promiscuous = is_promiscuous;
        self
    }

    #[instrument(skip_all)]
    pub fn build(self) -> NetworkCard {
        NetworkCard {
            addr: NetworkCardRaw::new(self.core, self.ip, self.mac, self.is_promiscuous).start(),
            ip: self.ip,
            mac: self.mac,
        }
    }
}

// Network Interface Cardなので、Ethernet以外でも使えるようにすべき
// もちろんEthernetFrameを返すのも変な話
struct NetworkCardRaw {
    eth: EthernetCard,
    ip: Ipv4Addr,
    rx_buffer: Arc<Mutex<VecDeque<EthernetFrame>>>,
}

impl NetworkCardRaw {
    fn new(core: Core, ip: Ipv4Addr, mac: MacAddr6, is_promiscuous: bool) -> Self {
        NetworkCardRaw {
            eth: EthernetCard::new(core, mac, is_promiscuous),
            ip,
            rx_buffer: Arc::new(Mutex::new(VecDeque::new())),
        }
    }
}

impl Actor for NetworkCardRaw {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let eth = self.eth.clone();
        let rx_buffer = self.rx_buffer.clone();

        ctx.spawn(
            async move {
                loop {
                    if let Some(ef) = eth.recv().await {
                        rx_buffer.lock().await.push_back(ef);
                    }
                }
            }
            .into_actor(self),
        );
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct NetworkCardRawSend {
    dst: MacAddr6,
    payload: EthernetFrameType,
}

impl Handler<NetworkCardRawSend> for NetworkCardRaw {
    type Result = ();

    fn handle(&mut self, msg: NetworkCardRawSend, ctx: &mut Self::Context) -> Self::Result {
        self.eth.send(msg.dst, msg.payload);
    }
}

#[derive(Message)]
#[rtype(result = "Option<EthernetFrame>")]
struct NetworkCardRawRecv;

impl Handler<NetworkCardRawRecv> for NetworkCardRaw {
    type Result = ResponseFuture<Option<EthernetFrame>>;

    fn handle(&mut self, msg: NetworkCardRawRecv, ctx: &mut Self::Context) -> Self::Result {
        let rx_buffer = self.rx_buffer.clone();

        Box::pin(async move { rx_buffer.lock().await.pop_front() })
    }
}

#[derive(Message)]
#[rtype(result = "Uuid")]
struct NetworkCardRawGetUuid;
impl Handler<NetworkCardRawGetUuid> for NetworkCardRaw {
    type Result = ResponseFuture<Uuid>;

    fn handle(&mut self, msg: NetworkCardRawGetUuid, ctx: &mut Self::Context) -> Self::Result {
        let eth = self.eth.clone();

        Box::pin(async move { eth.uuid().await })
    }
}

pub struct NetworkDriver {
    addr: Addr<NetworkDriverRaw>,
}

impl NetworkDriver {
    pub fn new(nic: NetworkCard) -> Self {
        NetworkDriver {
            addr: NetworkDriverRaw::new(nic).start(),
        }
    }

    pub fn send(&self, dst: Ipv4Addr, payload: IPv4PacketType) {
        self.addr.do_send(NetworkDriverSend { dst, payload })
    }

    pub async fn recv(&self) -> Option<IPv4Packet> {
        self.addr.send(NetworkDriverRecv {}).await.unwrap()
    }
}

impl Connectable for NetworkDriver {
    async fn uuid(&self) -> Uuid {
        self.addr.send(NetworkDriverGetUuid {}).await.unwrap()
    }
}

struct NetworkDriverRaw {
    nic: NetworkCard,
    arp_cache: Arc<Mutex<HashMap<Ipv4Addr, MacAddr6>>>,
    tx_pendings: Arc<Mutex<HashMap<Ipv4Addr, Vec<NetworkDriverSend>>>>,
    rx_buffer: Arc<Mutex<VecDeque<IPv4Packet>>>,
}

impl NetworkDriverRaw {
    fn new(nic: NetworkCard) -> Self {
        NetworkDriverRaw {
            nic,
            arp_cache: Arc::new(Mutex::new(HashMap::new())),
            tx_pendings: Arc::new(Mutex::new(HashMap::new())),
            rx_buffer: Arc::new(Mutex::new(VecDeque::new())),
        }
    }
}

impl Actor for NetworkDriverRaw {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let nic = self.nic.clone();
        let arp_cache = self.arp_cache.clone();
        let tx_pendings = self.tx_pendings.clone();
        let rx_buffer = self.rx_buffer.clone();
        let me = ctx.address().recipient();

        ctx.spawn(
            async move {
                loop {
                    if let Some(ef) = nic.recv().await {
                        match ef.payload {
                            EthernetFrameType::IPv4(packet) => {
                                rx_buffer.lock().await.push_back(packet);
                            }
                            EthernetFrameType::Arp(arp) => match arp.op {
                                ArpOperation::Reply => {
                                    if arp.dst_ip == nic.ip {
                                        arp_cache.lock().await.insert(arp.src_ip, arp.src_mac);

                                        if let Some(pendings) =
                                            tx_pendings.lock().await.remove(&arp.src_ip)
                                        {
                                            for pending in pendings.into_iter() {
                                                me.do_send(pending);
                                            }
                                        }
                                    }
                                }
                                ArpOperation::Request => {
                                    if arp.dst_ip == nic.ip {
                                        arp_cache.lock().await.insert(arp.src_ip, arp.src_mac);

                                        let arp_reply = ArpPacket::mk_reply(
                                            arp.src_ip,
                                            arp.src_mac,
                                            nic.ip,
                                            nic.mac,
                                        );

                                        nic.send(arp.dst_mac, EthernetFrameType::Arp(arp_reply));
                                    }
                                }
                            },
                            EthernetFrameType::Dummy => {}
                        }
                    }

                    tokio::task::yield_now().await;
                }
            }
            .into_actor(self),
        );
    }
}

#[derive(Message, Clone)]
#[rtype(result = "()")]
struct NetworkDriverSend {
    dst: Ipv4Addr,
    payload: IPv4PacketType,
}

impl Handler<NetworkDriverSend> for NetworkDriverRaw {
    type Result = ResponseFuture<()>;

    fn handle(&mut self, msg: NetworkDriverSend, ctx: &mut Self::Context) -> Self::Result {
        let arp_cache = self.arp_cache.clone();
        let tx_pendings = self.tx_pendings.clone();
        let nic = self.nic.clone();

        Box::pin(async move {
            if let Some(target_mac) = arp_cache.lock().await.get(&msg.dst) {
                nic.send(
                    *target_mac,
                    EthernetFrameType::IPv4(IPv4Packet::new(nic.ip, msg.dst, msg.payload)),
                );
            } else {
                // tx_pendings.lock().await.insert(msg.dst, msg.clone());
                tx_pendings
                    .lock()
                    .await
                    .entry(msg.dst)
                    .and_modify(|tbl| tbl.push(msg.clone()))
                    .or_insert(vec![msg.clone()]);

                nic.send(
                    MacAddr6::broadcast(),
                    EthernetFrameType::Arp(ArpPacket::mk_request(msg.dst, nic.ip, nic.mac)),
                );
            }
        })
    }
}

#[derive(Message)]
#[rtype(result = "Option<IPv4Packet>")]
struct NetworkDriverRecv;

impl Handler<NetworkDriverRecv> for NetworkDriverRaw {
    type Result = ResponseFuture<Option<IPv4Packet>>;

    fn handle(&mut self, msg: NetworkDriverRecv, ctx: &mut Self::Context) -> Self::Result {
        let rx_buffer = self.rx_buffer.clone();

        Box::pin(async move { rx_buffer.lock().await.pop_front() })
    }
}

#[derive(Message)]
#[rtype(result = "Uuid")]
struct NetworkDriverGetUuid;

impl Handler<NetworkDriverGetUuid> for NetworkDriverRaw {
    type Result = ResponseFuture<Uuid>;

    fn handle(&mut self, msg: NetworkDriverGetUuid, ctx: &mut Self::Context) -> Self::Result {
        let nic = self.nic.clone();

        Box::pin(async move { nic.uuid().await })
    }
}
