use actix::prelude::*;
use macaddr::MacAddr6;

#[derive(Debug, Clone, MessageResponse)]
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

#[derive(Debug, Clone, MessageResponse)]
pub enum EthernetFrameType {
    Dummy,
}
