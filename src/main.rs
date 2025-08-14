use macaddr::MacAddr6;
use std::collections::HashMap;
use std::future::Future;
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::{thread, time};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::{oneshot, Mutex};

use vnet::core::*;
use vnet::format::*;
use vnet::l2::*;
use vnet::router::*;

pub struct D {
    port: Device,
    mac: MacAddr6,
}

impl D {
    pub fn new(world: &mut World, mac: MacAddr6) -> Self {
        D {
            port: Device::new(world.uuid(), world.tx.clone()),
            mac,
        }
    }

    pub async fn send(&mut self, dst: MacAddr6, payload: Format) {
        self.port
            .send(EthernetFrame::new(self.mac, dst, payload))
            .await;
    }

    pub async fn recv(&mut self) -> EthernetFrame<Format> {
        loop {
            let frame = self.port.recv().await;

            // プロミスキャスの有効化
            if frame.header.dst == self.mac {
                return frame;
            } else if frame.header.dst.is_broadcast() {
                return frame;
            }
        }
    }
}

impl HasUuid for D {
    fn uuid(&self) -> Uuid {
        self.port.uuid
    }
}

// デバイスだろうが何だろうが、ネットワークレイヤレベルでArpなどを裏でやってくれるドライバみたいなのがいる

#[tokio::main]
async fn main() {
    let mut world = World::new();

    let mut router = Router::new(&mut world)
        .eth(
            0,
            [0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            [192, 168, 1, 254],
            [255, 255, 255, 0],
        )
        .eth(
            1,
            [0x00, 0x00, 0x00, 0x00, 0x00, 0x01],
            [192, 168, 2, 254],
            [255, 255, 255, 0],
        )
        .build();

    let mut l2 = L2::new(&mut world, 1);

    let dev0_mac = MacAddr6::new(0, 0, 0, 0, 0, 2);
    let dev1_mac = MacAddr6::new(0, 0, 0, 0, 0, 3);

    let mut dev0 = D::new(&mut world, dev0_mac);
    let mut dev1 = D::new(&mut world, dev1_mac);

    world.connect(&dev0, &router.ports.get(&0).unwrap().port);
    world.connect(&dev1, &router.ports.get(&1).unwrap().port);

    router.run();

    tokio::spawn(async move {
        dev0.send(
            MacAddr6::new(0, 0, 0, 0, 0, 0),
            Format::Ipv4(Ipv4Packet::new(
                Ipv4Addr::new(192, 168, 1, 1),
                Ipv4Addr::new(192, 168, 2, 1),
                "from dev 0 to dev 1!".to_string(),
            )),
        )
        .await;
    });

    tokio::spawn(async move {
        let r = dev1.recv().await;
        println!("received {:?}", r);
    });

    world.run().await;
}
