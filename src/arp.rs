use std::{collections::HashMap, net::Ipv4Addr};

use macaddr::MacAddr6;
use tracing::debug;

use crate::{ArpOperation, ArpPacket, EthernetFrameType, IPv4PacketType};

#[derive(Debug)]
pub struct ArpAction {
    pub send_frames: Vec<(MacAddr6, EthernetFrameType)>,
    pub updated_arp_cache: HashMap<Ipv4Addr, MacAddr6>,
    pub updated_tx_pendings: HashMap<Ipv4Addr, Vec<IPv4PacketType>>,
}

pub fn handle_arp(
    arp: &ArpPacket,
    my_ip: Ipv4Addr,
    my_mac: MacAddr6,
    mut arp_cache: HashMap<Ipv4Addr, MacAddr6>,
    mut tx_pendings: HashMap<Ipv4Addr, Vec<IPv4PacketType>>,
) -> ArpAction {
    let mut send_frames = vec![];

    match arp.op {
        ArpOperation::Reply => {
            debug!("reply arp");
            if arp.dst_ip == my_ip {
                arp_cache.insert(arp.src_ip, arp.src_mac);
                if let Some(pendings) = tx_pendings.remove(&arp.src_ip) {
                    for payload in pendings {
                        send_frames.push((
                            arp.src_mac,
                            EthernetFrameType::IPv4(crate::IPv4Packet::new(
                                my_ip, arp.src_ip, payload,
                            )),
                        ));
                    }
                }
            }
        },
        ArpOperation::Request => {
            debug!("request arp");
            if arp.dst_ip == my_ip {
                arp_cache.insert(arp.src_ip, arp.src_mac);
                let reply = ArpPacket::mk_reply(arp.src_ip, arp.src_mac, my_ip, my_mac);
                send_frames.push((arp.src_mac, EthernetFrameType::Arp(reply)));
            }
        },
    }

    ArpAction {
        send_frames,
        updated_arp_cache: arp_cache,
        updated_tx_pendings: tx_pendings,
    }
}
