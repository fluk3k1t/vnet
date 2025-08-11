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

    let mut l2 = L2::new(&mut world, 3);

    let dev0_mac = MacAddr6::new(0x00, 0x00, 0x00, 0x00, 0x00, 0x00);
    let dev1_mac = MacAddr6::new(0x00, 0x00, 0x00, 0x00, 0x00, 0x01);
    let dev2_mac = MacAddr6::new(0x00, 0x00, 0x00, 0x00, 0x00, 0x02);

    let dummy_ip = Ipv4Addr::new(0, 0, 0, 0);

    let mut dev0 = D::new(&mut world, dev0_mac);
    let mut dev1 = D::new(&mut world, dev1_mac);
    let mut dev2 = D::new(&mut world, dev2_mac);

    world.connect(l2.n_port(0).unwrap(), &dev0);
    world.connect(l2.n_port(1).unwrap(), &dev1);
    world.connect(l2.n_port(2).unwrap(), &dev2);

    tokio::spawn(async move {
        dev0.send(dev1_mac, Ipv4Packet::new(dummy_ip, dummy_ip, "from dev0".to_string())).await;
        let frame = dev0.port.recv().await;
        println!("dev0: received {:?}", frame);
        // da.port.send(Frame::new(Mac::new("damac"), Mac::new("dbmac"), "i'm received")).await;
        // da.port.send(Frame::new(Mac::new("damac"), Mac::new("dcmac"), "i'm received")).await;
    });

    tokio::spawn(async move {
        let frame = dev1.port.recv().await;
        println!("dev1: received {:?}", frame);
        dev1.send(dev0_mac, Ipv4Packet::new(dummy_ip, dummy_ip, "respond to dev0 from dev1".to_string())).await;
        // db.port.send(Frame::new(Mac::new("dbmac"), Mac::new("damac"), "response")).await;
        // println!("sended");
        // let r = db.port.recv().await;
        // println!("db: received {:?}", r);
    });

    tokio::spawn(async move {
        let frame = dev2.port.recv().await;
        println!("dev2: received {:?}", frame);
    });

    l2.run();

    world.run().await;
}