use std::collections::HashMap;
use std::future::Future;
use tokio::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::{thread, time};
use tokio::sync::{oneshot, Mutex};

type Packet = Frame;
type Uuid = u32;

#[derive(Clone, Debug, PartialEq)]
pub struct Mac {
    inner: String
}

impl Mac {
    pub fn new(dummy: impl Into<String>) -> Self {
        Mac {
            inner: dummy.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Frame {
    src: Mac,
    dst: Mac,
    data: String,
}

impl Frame {
    pub fn new(src: Mac, dst: Mac, data: impl Into<String>) -> Self {
        Frame {
            src,
            dst,
            data: data.into(),
        }
    }
}


pub enum Command {
    Send(Uuid, Packet),
    Recv(Uuid, oneshot::Sender<Packet>),
    RecvNonBlocking(Uuid, oneshot::Sender<Option<Packet>>),
}

pub struct World {
    tx: Sender<Command>,
    rx: Receiver<Command>,
    next_uuid: Uuid,
    pending: HashMap<Uuid, oneshot::Sender<Packet>>,
    buffer: HashMap<Uuid, Vec<Packet>>,
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

    pub fn create<T>(&mut self, obj: T) -> Model<T> {
        Model {
            inner: obj,
            uuid: self.uuid()
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
                Command::Send(src, packet) => {
                    if let Some(dsts) = self.connections.get(&src) {
                        for dst in dsts {
                            if let Some(dst_sender) = self.pending.remove(dst) {
                                dst_sender.send(packet.clone()).expect("Command Send failed");
                            } else {
                                if let Some(buffer) = self.buffer.get_mut(dst) {
                                    buffer.push(packet.clone());
                                } else {
                                    self.buffer.insert(*dst, vec![packet.clone()]);
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

    pub fn bind<T, F, Fut>(&mut self, model: Model<T>, prog: F)
    where
        T: Send + 'static,
        F: Fn(T, Device) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let dev = Device::new(model.uuid, self.tx.clone());

        tokio::spawn(async move {
            prog(model.inner, dev).await;
        });
    }
}

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

    pub async fn send(&mut self, packet: Packet) {
        self.tx.send(Command::Send(self.uuid, packet)).await.expect("command send failed");
    }

    pub async fn recv(&mut self) -> Packet {
        let (tx, rx) = oneshot::channel();

        self.tx.send(Command::Recv(self.uuid, tx)).await.expect("command recv failed"); 

        rx.await.expect("command recv callback failed")
    }

    // 表面的な挙動は同期的だがcoreにメッセージパッシングでコールするので非同期になっている
    pub async fn recv_nonblocking(&mut self) -> Option<Packet> {
        let (tx, rx) = oneshot::channel();

        self.tx.send(Command::RecvNonBlocking(self.uuid, tx)).await.expect("send command recv nonblocking failed");

        rx.await.expect("command recv nonblocking callback failed. unreachable!")
    }
}

pub struct Raw<T> {
    model: Model<T>,

}

pub struct LanAdapter {

}

pub struct Model<T> {
    inner: T,
    uuid: Uuid,
}

pub struct _L2 {
    mac: Mac,
}

impl _L2 { 
    pub fn new(mac: Mac) -> Self {
        _L2 {
            mac,
        }
    }
}

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
}

impl HasUuid for D {
    fn uuid(&self) -> Uuid {
        self.port.uuid
    }
}

pub struct L2 {
    ports: Vec<Device>,
    mactbl: HashMap<u32, Mac>,
}

impl L2 {
    pub fn new(world: &mut World, n_ports: u32) -> Self {
        L2 {
            ports: (0..n_ports).into_iter().map(|_| world.port()).collect(),
            mactbl: HashMap::new(),
        }   
    }

    pub fn n_port(&mut self, n_port: usize) -> Option<&Device> {
        self.ports.get(n_port)
    }
}

#[tokio::main]
async fn main() {
    let mut world = World::new();

    let mut l2 = L2::new(&mut world, 2);
    let mut da = D::new(&mut world, Mac::new("damac"));

    world.connect(l2.n_port(0).unwrap(), &da);

    da.port.send(Frame::new(Mac::new("damac"), Mac::new("todbmac"), "hey")).await;

    tokio::spawn(async move {
        let recvs: Vec<_> = {
            let mut tmp = Vec::new();
            for port in &mut l2.ports {
                if let Some(recv) = port.recv_nonblocking().await {
                    tmp.push(recv);
                }
            }
            tmp
        };

        for (n_port, recv) in recvs.into_iter().enumerate() {
            l2.mactbl.insert(n_port as u32, recv.src.clone());

            // Macアドレステーブルに学習済みのポートが存在する場合、そのポートにのみフレームを流す
            if let Some(dstport) = l2.mactbl.iter().find(|(_, mac)| **mac == recv.dst).map(|(n_port, _)| n_port) {
                l2.ports.get_mut(*dstport as usize).expect("l2 send failed: unreachable!").send(recv).await;
            } else {
                // 学習済みでない場合、すべてのポートにフレームを流す
                for port in l2.ports.iter_mut() {
                    port.send(recv.clone()).await;
                }
            }
        }

        // スレッドロックしないよう、tokioランタイムに制御を戻す
        tokio::task::yield_now().await;
    });

    world.run().await;
}