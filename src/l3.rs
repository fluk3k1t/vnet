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
use tracing::debug;

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

    pub fn send(&self, dst: MacAddr6, payload: EthernetFrameType) {
        self.addr.do_send(NetworkCardRawSend { dst, payload });
    }

    pub async fn recv(&self) -> Option<EthernetFrame> {
        self.addr.send(NetworkCardRawRecv).await.unwrap()
    }
}

impl NetworkCardBuilder {
    pub fn promiscuous(mut self, is_promiscuous: bool) -> Self {
        self.is_promiscuous = is_promiscuous;
        self
    }

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

pub struct NetworkDriver {}

struct NetworkDriverRaw {
    nic: NetworkCard,
    arp_cache: Arc<Mutex<HashMap<Ipv4Addr, MacAddr6>>>,
    tx_pendings: Arc<Mutex<HashMap<Ipv4Addr, NetworkDriverSend>>>,
}

impl NetworkDriverRaw {
    fn new(nic: NetworkCard) -> Self {
        NetworkDriverRaw {
            nic,
            arp_cache: Arc::new(Mutex::new(HashMap::new())),
            tx_pendings: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Actor for NetworkDriverRaw {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let nic = self.nic.clone();
        let arp_cache = self.arp_cache.clone();
        let tx_pendings = self.tx_pendings.clone();
        let me = ctx.address().recipient();

        ctx.spawn(
            async move {
                loop {
                    if let Some(ef) = nic.recv().await {
                        match ef.payload {
                            EthernetFrameType::IPv4(packet) => {}
                            EthernetFrameType::Arp(arp) => match arp.op {
                                ArpOperation::Reply => {
                                    if arp.dst_ip == nic.ip {
                                        arp_cache.lock().await.insert(arp.dst_ip, arp.dst_mac);

                                        if let Some(pending) =
                                            tx_pendings.lock().await.remove(&arp.dst_ip)
                                        {
                                            me.do_send(pending);
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
                }
            }
            .into_actor(self),
        );
    }
}

#[derive(Message)]
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
                tx_pendings.lock().await.insert(msg.dst, msg);
            }
        })
    }
}
