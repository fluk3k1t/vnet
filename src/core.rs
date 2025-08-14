use crate::format::*;

use std::collections::HashMap;
use std::future::Future;
use std::net::Ipv4Addr;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::oneshot;

pub type Uuid = u32;

pub trait HasUuid {
    fn uuid(&self) -> Uuid;
}

impl HasUuid for Device {
    fn uuid(&self) -> Uuid {
        self.uuid
    }
}

impl HasUuid for &Device {
    fn uuid(&self) -> Uuid {
        self.uuid
    }
}

pub enum Command {
    Send(Uuid, EthernetFrame<Ipv4Packet<String>>),
    Recv(Uuid, oneshot::Sender<EthernetFrame<Ipv4Packet<String>>>),
    RecvNonBlocking(Uuid, oneshot::Sender<Option<EthernetFrame<Ipv4Packet<String>>>>),
}

pub struct World {
    // 隠ぺいできるはず
    pub tx: Sender<Command>,
    rx: Receiver<Command>,
    next_uuid: Uuid,
    pending: HashMap<Uuid, oneshot::Sender<EthernetFrame<Ipv4Packet<String>>>>,
    buffer: HashMap<Uuid, Vec<EthernetFrame<Ipv4Packet<String>>>>,
    connections: HashMap<Uuid, Vec<Uuid>>,
}

impl World {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(32);
        World {
            tx,
            rx,
            next_uuid: 0,
            pending: HashMap::new(),
            buffer: HashMap::new(),
            connections: HashMap::new(),
        }
    }

    pub fn port(&mut self) -> Device {
        Device::new(self.uuid(), self.tx.clone())
    }

    pub fn connect(&mut self, m0: &impl HasUuid, m1: &impl HasUuid) {
        if let Some(m0tbl) = self.connections.get_mut(&m0.uuid()) {
            m0tbl.push(m1.uuid());
        } else {
            self.connections.insert(m0.uuid(), vec![m1.uuid()]);
        }

        if let Some(m1tbl) = self.connections.get_mut(&m1.uuid()) {
            m1tbl.push(m0.uuid());
        } else {
            self.connections.insert(m1.uuid(), vec![m0.uuid()]);
        }
    }

    pub fn uuid(&mut self) -> Uuid {
        let new_uuid = self.next_uuid;
        self.next_uuid += 1;
        return new_uuid;
    }

    pub async fn run(mut self) {
        while let Some(com) = self.rx.recv().await {
            match com {
                Command::Send(src, frame) => {
                    if let Some(dsts) = self.connections.get(&src) {
                        for dst in dsts {
                            if let Some(dst_sender) = self.pending.remove(dst) {
                                dst_sender.send(frame.clone()).expect("Command Send failed");
                            } else {
                                if let Some(buffer) = self.buffer.get_mut(dst) {
                                    buffer.push(frame.clone());
                                } else {
                                    self.buffer.insert(*dst, vec![frame.clone()]);
                                }
                            }
                        }   
                    }
                },
                Command::Recv(uuid, res) => {
                    if let Some(buffer) = self.buffer.get_mut(&uuid) {
                        if !buffer.is_empty() {
                            res.send(buffer.pop().unwrap()).expect("Command Recv failed");
                        } else {
                            self.pending.insert(uuid, res);
                        }
                    } else {
                        self.pending.insert(uuid, res);
                    }
                },
                Command::RecvNonBlocking(uuid, res) => {
                    if let Some(buffer) = self.buffer.get_mut(&uuid) {
                        res.send(buffer.pop()).expect("Command RecvNonblocking failed");
                    } else {
                        res.send(None).expect("Command RecvNonBlocking failed");
                    }
                },
                _ => todo!(),
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Device {
    pub uuid: Uuid,
    pub tx: Sender<Command>,
}

impl Device {
    pub fn new(uuid: Uuid, tx: Sender<Command>) -> Self {
        Device {
            uuid,
            tx, 
        }
    }

    pub async fn send(&mut self, frame: EthernetFrame<Ipv4Packet<String>>) {
        self.tx.send(Command::Send(self.uuid, frame)).await.expect("command send failed");
    }

    pub async fn recv(&mut self) -> EthernetFrame<Ipv4Packet<String>> {
        let (tx, rx) = oneshot::channel();

        self.tx.send(Command::Recv(self.uuid, tx)).await.expect("command recv failed"); 

        rx.await.expect("command recv callback failed")
    }

    // 表面的な挙動は同期的だがcoreにメッセージパッシングでコールするので非同期になっている
    pub async fn recv_nonblocking(&mut self) -> Option<EthernetFrame<Ipv4Packet<String>>> {
        let (tx, rx) = oneshot::channel();

        self.tx.send(Command::RecvNonBlocking(self.uuid, tx)).await.expect("send command recv nonblocking failed");

        rx.await.expect("command recv nonblocking callback failed. unreachable!")
    }
}

pub struct Ethernet {
    port: Device,
    ip: Ipv4Addr,
}

impl Ethernet {
    pub fn new() -> Self {
        todo!()
    }
}