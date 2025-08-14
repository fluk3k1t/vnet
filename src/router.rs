use macaddr::MacAddr6;

use crate::core::*;
use crate::format::*;

use std::collections::HashMap;
use std::net::Ipv4Addr;

#[derive(Debug, Clone)]
struct PortConfig {
    ip: Ipv4Addr,
    subnet: Ipv4Addr,
    port: Device,
    // デバイスに紐づいていたほうが自然な気はする
    // ただ現状デバイスはシミュレーション世界での通信路の抽象化なので、もう一段何かしらを挟んだほうが適切な気はする
    mac: MacAddr6,
}

pub struct Router {
    ports: HashMap<u32, PortConfig>,
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

    pub fn run(mut self) {
        tokio::spawn(async move {
            loop {
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
                    if let Some((send_to_com, send_to_port)) = self.ports.iter().find(|(n, port)| port.ip & port.subnet == recv.payload.header.dst & port.subnet) {
                        
                    }
                }
            }
        });
    }
}

impl<'a> RouterBuilder<'a> {
    pub fn eth(self, com: u32, mac: impl Into<MacAddr6>, ip: impl Into<Ipv4Addr>, subnet: impl Into<Ipv4Addr>) -> Self {
        // cloneかmut selfか.速度を見るなら明らかにmutだが、、、
        let mut ports = self.ports.clone();
        ports.insert(com, PortConfig { ip: ip.into(), subnet: subnet.into(), port: self.world.port(), mac: mac.into() });

        RouterBuilder {
            world: self.world,
            ports,
        }
    }

    pub fn build(self) -> Router {
        Router {
            ports: HashMap::new(),
        }
    }
}