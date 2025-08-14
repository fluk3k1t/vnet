use std::collections::HashMap;
use std::future::Future;
use std::net::Ipv4Addr;
use macaddr::MacAddr6;
use tokio::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::{thread, time};
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

    pub async fn send(&mut self, dst: MacAddr6, payload: Ipv4Packet<String>) {
        self.port.send(EthernetFrame::new(self.mac, dst, payload)).await;
    }

    pub async fn recv(&mut self) -> EthernetFrame<Ipv4Packet<String>> {
        loop {
            let frame = self.port.recv().await;
            if frame.header.dst == self.mac {
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


#[tokio::main]
async fn main() {
    let mut world = World::new();

    let mut router = Router::new(&mut world)
                        .eth(0, [0x00, 0x00, 0x00, 0x00, 0x00, 0x00], [192, 168, 1, 254], [255, 255, 255, 0])
                        .eth(1, [0x00, 0x00, 0x00, 0x00, 0x00, 0x01], [192, 168, 2, 254], [255, 255, 255, 0])
                        .build();

    let mut l2 = L2::new(&mut world, 1);

    tokio::spawn(async {
        router.run();
    });

    world.run().await;
}