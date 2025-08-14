use macaddr::{MacAddr, MacAddr6};
use std::net::Ipv4Addr;

use crate::ArpPacket;

#[derive(Clone, Debug, PartialEq)]
pub struct EthernetFrame<T> {
    pub header: EthernetHeader,
    pub payload: T,
}

impl<T> EthernetFrame<T> {
    pub fn new(src: MacAddr6, dst: MacAddr6, payload: T) -> EthernetFrame<T> {
        EthernetFrame {
            header: EthernetHeader { src, dst },
            payload,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EthernetHeader {
    pub src: MacAddr6,
    pub dst: MacAddr6,
}

#[derive(Clone, Debug)]
pub struct Ipv4Packet<T> {
    pub header: Ipv4Header,
    pub payload: T,
}

impl<T> Ipv4Packet<T> {
    pub fn new(src: Ipv4Addr, dst: Ipv4Addr, payload: T) -> Ipv4Packet<T> {
        Ipv4Packet {
            header: Ipv4Header { src, dst },
            payload,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Ipv4Header {
    pub src: Ipv4Addr,
    pub dst: Ipv4Addr,
}

#[derive(Debug, Clone)]
pub enum Format {
    Ipv4(Ipv4Packet<String>),
    Arp(ArpPacket),
}
