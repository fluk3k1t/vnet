use macaddr::MacAddr6;

use crate::arp::*;
use crate::core::*;
use crate::format::*;

use std::collections::HashMap;
use std::collections::VecDeque;
use std::net::Ipv4Addr;

#[derive(Debug, Clone)]
struct PortConfig {
    ip: Ipv4Addr,
    subnet: Ipv4Addr,
    // というかネットワーク層から見てデータリンク層のハンドラが欲しい
    port: Device,
    // デバイスに紐づいていたほうが自然な気はする
    // ただ現状デバイスはシミュレーション世界での通信路の抽象化なので、もう一段何かしらを挟んだほうが適切な気はする
    mac: MacAddr6,
}

#[derive(Debug, Clone)]
pub struct Message {
    com: u32,
    data: Format,
    src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
}

// VStreamへのsend自体は同期的ノンブロッキングだけど、セッション的には（レイヤーの話ではない）arpしたりと非同期になることもあるのでjob queueにする
// Router固有ではないので、LanAdapterライブラリ的なレイヤのものとしてのちのち実装したい
pub struct Router {
    ports: HashMap<u32, PortConfig>,
    arp_table: HashMap<Ipv4Addr, MacAddr6>,
    message_fifo: VecDeque<Message>,
    arp_requests: Vec<Ipv4Addr>,
}

pub struct RouterBuilder<'a> {
    world: &'a mut World,
    ports: HashMap<u32, PortConfig>,
}

impl Router {
    pub fn new(world: &mut World) -> RouterBuilder {
        RouterBuilder {
            world,
            ports: HashMap::new(),
        }
    }

    pub async fn get_mac_from_ip(&mut self, ip: Ipv4Addr) -> MacAddr6 {
        if let Some(cached_mac) = self.arp_table.get(&ip) {
            return *cached_mac;
        } else {
            todo!()
        }
    }

    pub fn send(&mut self, com: u32, src_ip: Ipv4Addr, dst_ip: Ipv4Addr, data: Format) {
        self.message_fifo.push_back(Message {
            com,
            data,
            src_ip,
            dst_ip,
        });
    }

    pub async fn arp(&mut self, com: u32, ip: Ipv4Addr) -> MacAddr6 {
        // macはブロードキャスト
        let port = self.ports.get_mut(&com).unwrap();

        let arp_req_packet = ArpPacket::mk_request(ip, port.ip, port.mac);

        port.port.send(EthernetFrame::new(
            port.mac,
            MacAddr6::broadcast(),
            Format::Arp(arp_req_packet),
        ));

        todo!()
    }

    pub fn run(mut self) {
        // 受信と送信を同じスレッドでやると、受信まちのブロックで送信が阻害されて～みていなバグが起こりそう
        // ブロッキングの非同期使わなければいいだけか。n portsの時点で非同期にはならない
        // 現状moveなので同じスレッド。問題が起きたらarcするしかない
        tokio::spawn(async move {
            loop {
                // send
                // handle suspended_send_message
                for (idx, message) in self.message_fifo.clone().iter().enumerate() {
                    if let Some(cached_dst_mac) = self.arp_table.get(&message.dst_ip) {
                        let port = &mut self.ports.get_mut(&message.com).unwrap();

                        port.port
                            .send(EthernetFrame::new(
                                port.mac,
                                *cached_dst_mac,
                                message.data.clone(),
                            ))
                            .await;

                        self.message_fifo.remove(idx);
                    } else {
                        if !self.arp_requests.contains(&message.dst_ip) {
                            self.arp_requests.push(message.dst_ip);
                        }
                    }
                }

                // receive
                let recvs: Vec<_> = {
                    let mut tmp = Vec::new();

                    for (n, port) in self.ports.iter_mut() {
                        if let Some(recv) = port.port.recv_nonblocking().await {
                            // *nをnにするとどうなるか考えてみて。nはself.ports.iter_mut()からの参照で、コピーしないと可変参照が残ったままになるので、のちのコードでself.portsが借用できなくなる
                            tmp.push((*n, recv));
                        }
                    }

                    tmp
                };

                for (n, recv) in recvs {
                    // 受信データのipパケットの宛先ipと同じネットワークに属するポートを検索
                    // findなので1ポートしか選択できないが、おそらくルーターでは１つのネットワークに存在していいポートは１つだけのはず
                    match recv.payload {
                        Format::Ipv4(payload) => {
                            if let Some((send_to_com, send_to_port)) =
                                self.ports.iter().find(|(n, port)| {
                                    port.ip & port.subnet == payload.header.dst & port.subnet
                                })
                            {
                                self.send(
                                    *send_to_com,
                                    send_to_port.ip,
                                    payload.header.dst,
                                    Format::Ipv4(payload),
                                );
                            }
                        }
                        Format::Arp(payload) => match payload.op {
                            ArpOperation::Reply => {
                                self.arp_table.insert(payload.dst_ip, payload.dst_mac);
                            }
                            _ => unreachable!(),
                        },
                        _ => todo!(),
                    }
                }
            }
        });
    }
}

impl<'a> RouterBuilder<'a> {
    pub fn eth(
        self,
        com: u32,
        mac: impl Into<MacAddr6>,
        ip: impl Into<Ipv4Addr>,
        subnet: impl Into<Ipv4Addr>,
    ) -> Self {
        // cloneかmut selfか.速度を見るなら明らかにmutだが、、、
        let mut ports = self.ports.clone();
        ports.insert(
            com,
            PortConfig {
                ip: ip.into(),
                subnet: subnet.into(),
                port: self.world.port(),
                mac: mac.into(),
            },
        );

        RouterBuilder {
            world: self.world,
            ports,
        }
    }

    pub fn build(self) -> Router {
        Router {
            ports: HashMap::new(),
            arp_table: HashMap::new(),
            message_fifo: VecDeque::new(),
            arp_requests: Vec::new(),
        }
    }
}
