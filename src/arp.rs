use macaddr::MacAddr6;

use crate::core::*;
use crate::format::*;

use std::collections::HashMap;
use std::net::Ipv4Addr;

// そもそもヘッダの構造としては現状のほうが違いが、Rustの思想的にはenumをtagとして持つよりenum自体にデータを持たせるのが適切か？？
// まあ直和であることに変わりはないからいいか。このほうが直観的だし

// 本来はL2, L3のプロトコルタイプや他のタイプ指定があるが、バイトエンコーディングするつもりはないのでパターンマッチ
#[derive(Debug, Clone)]
pub struct ArpPacket {
    // Arpとは別に、L2パケットにもmac情報が載る
    pub src_mac: MacAddr6,
    // Arp Requestの場合は00000
    pub dst_mac: MacAddr6,
    pub src_ip: Ipv4Addr,
    pub dst_ip: Ipv4Addr,
    pub op: ArpOperation,
}

#[derive(Debug, Clone)]
pub enum ArpOperation {
    Request,
    Reply,
}

impl ArpPacket {
    pub fn mk_request(target_ip: Ipv4Addr, from_ip: Ipv4Addr, from_mac: MacAddr6) -> ArpPacket {
        ArpPacket {
            src_mac: from_mac,
            dst_mac: MacAddr6::new(0x00, 0x00, 0x00, 0x00, 0x00, 0x00),
            dst_ip: target_ip,
            src_ip: from_ip,
            op: ArpOperation::Request,
        }
    }
}
