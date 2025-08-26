use std::net::Ipv4Addr;

use macaddr::MacAddr6;

use crate::{Core, EthernetCard, EthernetFrameType, IPv4PacketType};

pub struct NetworkCard {
    ip: Ipv4Addr,
    eth: EthernetCard,
}

impl NetworkCard {
    pub fn new(core: &mut Core, ip: Ipv4Addr, mac: MacAddr6, is_promiscuous: bool) -> Self {
        NetworkCard {
            ip,
            eth: EthernetCard::new(core, mac, is_promiscuous),
        }
    }

    pub fn send(&mut self, payload: EthernetFrameType) {}
}

pub struct NetworkDriver {
    nic: NetworkCard,
}

impl NetworkDriver {
    pub fn new(nic: NetworkCard) -> Self {
        NetworkDriver { nic }
    }

    pub fn send(&mut self, dst: Ipv4Addr, payload: IPv4PacketType) {
        
    }
}
