use std::net::Ipv4Addr;

use macaddr::MacAddr6;

use crate::Ethernet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EthernetFrame {
    pub dst: MacAddr6,
    pub src: MacAddr6,
    pub payload: EthernetFrameType,
}

impl EthernetFrame {
    pub fn new(src: MacAddr6, dst: MacAddr6, payload: EthernetFrameType) -> Self {
        EthernetFrame { src, dst, payload }
    }

    pub fn dummy() -> Self {
        EthernetFrame {
            src: MacAddr6::nil(),
            dst: MacAddr6::nil(),
            payload: EthernetFrameType::Debug("dummy".to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EthernetFrameType {
    IPv4(IPv4Packet),
    Debug(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IPv4Packet {
    src: Ipv4Addr,
    dst: Ipv4Addr,
    payload: IPv4PacketType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IPv4PacketType {
    ICMP,
    TCP,
    UDP,
    Debug(String),
}
