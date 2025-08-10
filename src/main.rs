use std::collections::HashMap;
use std::future::Future;
use tokio::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::{thread, time};
use tokio::sync::{oneshot, Mutex};

use vnet::core::*;
use vnet::format::*;
use vnet::l2::*;

pub struct D {
    port: Device,
    mac: Mac,
}

impl D {
    pub fn new(world: &mut World, mac: Mac) -> Self {
        D {
            port: Device::new(world.uuid(), world.tx.clone()),
            mac,
        }
    }

    pub async fn send(&mut self, frame: Frame) {
        self.port.send(frame).await;
    }

    pub async fn recv(&mut self) -> Frame {
        loop {
            let frame = self.port.recv().await;
            if frame.dst == self.mac {
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
    let mut da = D::new(&mut world, Mac::new("damac"));
    let mut db = D::new(&mut world, Mac::new("dbmac"));
    let mut dc = D::new(&mut world, Mac::new("dcmac"));

    world.connect(l2.n_port(0).unwrap(), &da);
    world.connect(l2.n_port(1).unwrap(), &db);
    world.connect(l2.n_port(2).unwrap(), &dc);

    tokio::spawn(async move {
        da.port.send(Frame::new(Mac::new("damac"), Mac::new("dbmac"), "hey")).await;
        let r = da.port.recv().await;
        println!("da: received {:?}", r);
        da.port.send(Frame::new(Mac::new("damac"), Mac::new("dbmac"), "i'm received")).await;
        da.port.send(Frame::new(Mac::new("damac"), Mac::new("dcmac"), "i'm received")).await;
    });

    tokio::spawn(async move {
        let r = db.port.recv().await;
        println!("db: received {:?}", r);
        db.port.send(Frame::new(Mac::new("dbmac"), Mac::new("damac"), "response")).await;
        println!("sended");
        let r = db.port.recv().await;
        println!("db: received {:?}", r);
    });

    tokio::spawn(async move {
        let r = dc.recv().await;
        println!("dc: received {:?}", r);
    });

    l2.run();

    world.run().await;
}