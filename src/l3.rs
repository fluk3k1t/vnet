use std::{
    collections::{HashMap, VecDeque},
    net::Ipv4Addr,
    sync::Arc,
};

use macaddr::MacAddr6;
use tokio::sync::{
    Mutex,
    mpsc::{self, Receiver, Sender},
};

use crate::{
    ArpOperation, ArpPacket, Core, EthernetCard, EthernetFrame, EthernetFrameType, IPv4Packet,
    IPv4PacketType,
};

pub struct NetworkCard {
    pub ip: Ipv4Addr,
    pub eth: EthernetCard,
}

impl NetworkCard {
    pub fn new(core: &mut Core, ip: Ipv4Addr, mac: MacAddr6, is_promiscuous: bool) -> Self {
        NetworkCard {
            ip,
            eth: EthernetCard::new(core, mac, is_promiscuous),
        }
    }

    pub async fn send(&mut self, dst: MacAddr6, payload: EthernetFrameType) {
        self.eth.send(dst, payload).await;
    }

    pub async fn recv(&mut self) -> Option<EthernetFrameType> {
        self.eth.recv().await.map(|ef| ef.payload)
    }
}

pub struct NetworkDriver {
    rx_buffer: Arc<Mutex<VecDeque<IPv4Packet>>>,
    tx_sender: Sender<(Ipv4Addr, IPv4PacketType)>,
    arp_cache: Arc<Mutex<HashMap<Ipv4Addr, MacAddr6>>>,
}

impl NetworkDriver {
    pub fn new(mut nic: NetworkCard) -> Self {
        let rx_buffer = Arc::new(Mutex::new(VecDeque::new()));
        let _rx_buffer = rx_buffer.clone();

        let (tx_sender, mut tx_receiver) = mpsc::channel(32);
        let _tx_sender = tx_sender.clone();

        let arp_cache = Arc::new(Mutex::new(HashMap::new()));
        let _arp_cache = arp_cache.clone();

        let mut arp_requested = HashMap::new();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    ef = nic.recv() => if let Some(ef) = ef {
                        match ef {
                            EthernetFrameType::IPv4(packet) => {
                                if packet.dst == nic.ip {
                                    _rx_buffer.lock().await.push_back(packet);
                                }
                            },
                            EthernetFrameType::Arp(arp) => {
                                match arp.op {
                                    ArpOperation::Reply => {
                                        if arp.dst_mac == nic.eth.mac {
                                            if let Some(requested_payload) = arp_requested.remove(&arp.dst_ip) {
                                                _arp_cache.lock().await.insert(arp.src_ip, arp.src_mac);
                                                _tx_sender.send((arp.src_ip, requested_payload)).await.unwrap();
                                            }
                                        }
                                    },
                                    ArpOperation::Request => {
                                        if arp.dst_ip == nic.ip {
                                            let arp_reply = ArpPacket::mk_reply(arp.src_ip, arp.src_mac, nic.ip, nic.eth.mac);
                                            // _tx_sender.send((arp.src_ip, IPv4Packet::Arp(arp_reply))).await.unwrap();
                                        }
                                    },
                                }
                            },
                            _ => unimplemented!(),
                        }
                    },
                    cmd = tx_receiver.recv() => if let Some((dst_ip, payload)) = cmd {
                        if let Some(dst_mac) = _arp_cache.lock().await.get(&dst_ip) {
                            let ef = EthernetFrameType::IPv4(IPv4Packet::new(nic.ip, dst_ip, payload));
                            nic.send(*dst_mac, ef).await;
                        } else {
                            arp_requested.insert(dst_ip, payload);
                        }
                    }
                }
            }
        });

        tokio::spawn(async move {});

        NetworkDriver {
            rx_buffer,
            tx_sender,
            arp_cache,
        }
    }

    pub async fn send(&mut self, dst: Ipv4Addr, payload: IPv4PacketType) {
        self.tx_sender.send((dst, payload)).await.unwrap();
    }

    pub async fn recv(&mut self) -> Option<IPv4Packet> {
        self.rx_buffer.lock().await.pop_front()
    }
}
