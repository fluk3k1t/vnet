use std::collections::HashMap;
use tokio::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::{thread, time};
use tokio::sync::Mutex;

type Packet = String;

pub struct World {
    pub connections: HashMap<u32, Vec<u32>>,
    // Worldを挟む通信が必要になるかわからんからsenderもreceiverも作っとく
    pub senders: HashMap<u32, Sender<Packet>>,
    pub receivers: HashMap<u32, Receiver<Packet>>,
    pub next_id: u32,
}

impl World {
    pub fn new() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(
            World {
                connections: HashMap::new(),
                senders: HashMap::new(),
                receivers: HashMap::new(),
                next_id: 0,
            }
        ))
    }

    pub async fn connect(_self: Arc<Mutex<Self>>, d0: &LanAdapter, d1: &LanAdapter) {
        let mut _self = _self.lock().await;

        if let Some(d0tbl) = _self.connections.get_mut(&d0.uuid) {
            d0tbl.push(d1.uuid);
        } else {
            _self.connections.insert(d0.uuid, vec![d1.uuid]);

            let (tx, rx) = mpsc::channel(32);
            _self.senders.insert(d0.uuid, tx);
            _self.receivers.insert(d0.uuid, rx);
        }

        if let Some(d1tbl) = _self.connections.get_mut(&d1.uuid) {
            d1tbl.push(d0.uuid);
        } else {
            _self.connections.insert(d1.uuid, vec![d0.uuid]);

            let (tx, rx) = mpsc::channel(32);
            _self.senders.insert(d1.uuid, tx);
            _self.receivers.insert(d1.uuid, rx);
        }
    }

    pub async fn bind(_self: Arc<Mutex<Self>>, uuid: u32) -> Eth {
        Eth::new(_self.clone(), _self.lock().await.receivers.remove(&uuid).unwrap(), uuid)
    }

    pub async fn send(_self: Arc<Mutex<Self>>, packet: Packet, src: u32) {
        let mut _self = _self.lock().await;
        let targets: Vec<u32> = _self.connections
            .get(&src)
            .cloned() 
            .unwrap_or_default();

        for target in targets {
            if let Some(sender) = _self.senders.get_mut(&target) {
                sender.send(packet.clone()).await.expect("unreachable");
            } else {
                panic!("not found");
            }
        }
    }

    pub async fn uuid(_self: Arc<Mutex<Self>>) -> u32 {
        let mut _self = _self.lock().await;
        let uuid = _self.next_id;
        _self.next_id += 1;
        uuid
    }
}

pub struct LanAdapter {
    pub uuid: u32,
}

impl LanAdapter {
    pub async fn new(world: Arc<Mutex<World>>) -> Self {
        LanAdapter {
            uuid: World::uuid(world).await,
        } 
    }
}

pub struct Eth {
    pub world: Arc<Mutex<World>>,
    pub receiver: Receiver<Packet>,
    pub uuid: u32,
}

impl Eth {
    pub fn new(world: Arc<Mutex<World>>, receiver: Receiver<Packet>, uuid: u32) -> Self {
        Eth {
            world,
            receiver,
            uuid,
        }
    }

    pub async fn recv(&mut self) -> Packet {
        self.receiver.recv().await.unwrap()
    }

    pub async fn send(&mut self, packet: Packet) {
        // cloneですって???
        World::send(self.world.clone(), packet, self.uuid).await;
    }
}

#[tokio::main]
async fn main() {
    let world = World::new();

    let _lan0 = LanAdapter::new(world.clone()).await;
    let _lan1 = LanAdapter::new(world.clone()).await;

    World::connect(world.clone(), &_lan0, &_lan1).await;

    let mut world1 = world.clone();
    tokio::spawn(async move {
        let mut lan1 = World::bind(world1.clone(), _lan1.uuid).await;

        println!("lan1 recv");
        let d = lan1.recv().await;
        println!("received {}", d);
    });


    let mut world0 = world.clone();
    tokio::spawn(async move {
        let mut lan0 = World::bind(world0.clone(), _lan0.uuid).await;
        
        println!("lan0 send");
        lan0.send("from_lan0".to_string()).await;
    });

    

    thread::sleep(time::Duration::from_secs(5));
}