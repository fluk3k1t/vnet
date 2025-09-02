use std::net::Ipv4Addr;

use macaddr::MacAddr6;

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EthernetFrameType {
    Dummy,
    IPv4(IPv4Packet),
    Arp(ArpPacket),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IPv4Packet {
    pub src: Ipv4Addr,
    pub dst: Ipv4Addr,
    pub payload: IPv4PacketType,
}

impl IPv4Packet {
    pub fn new(src: Ipv4Addr, dst: Ipv4Addr, payload: IPv4PacketType) -> Self {
        IPv4Packet { src, dst, payload }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IPv4PacketType {
    Debug(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArpPacket {
    pub dst_mac: MacAddr6,
    pub src_mac: MacAddr6,
    pub src_ip: Ipv4Addr,
    pub dst_ip: Ipv4Addr,
    pub op: ArpOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArpOperation {
    Request,
    Reply,
}

impl ArpPacket {
    pub fn mk_reply(
        from_ip: Ipv4Addr,
        from_mac: MacAddr6,
        self_ip: Ipv4Addr,
        self_mac: MacAddr6,
    ) -> Self {
        ArpPacket {
            op: ArpOperation::Reply,
            dst_mac: from_mac,
            dst_ip: from_ip,
            src_mac: self_mac,
            src_ip: self_ip,
        }
    }

    pub fn mk_request(target_ip: Ipv4Addr, self_ip: Ipv4Addr, self_mac: MacAddr6) -> Self {
        ArpPacket {
            op: ArpOperation::Request,
            dst_mac: MacAddr6::new(0x00, 0x00, 0x00, 0x00, 0x00, 0x00),
            dst_ip: target_ip,
            src_mac: self_mac,
            src_ip: self_ip,
        }
    }
}

