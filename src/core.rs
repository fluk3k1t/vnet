use anyhow::{Context, Result, ensure};
use std::collections::HashMap;
use tokio::sync::mpsc::{self, Receiver, Sender};

pub type Uuid = u32;

pub struct Message {
    uuid: Uuid,
    payload: Stream,
}

pub struct Core {
    rx: Receiver<Message>,
    tx: Sender<Message>,
    coms: HashMap<Uuid, Sender<Stream>>,
    connections: HashMap<Uuid, Vec<Uuid>>,
    next_uuid: Uuid,
}

impl Core {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(32);
        Core {
            tx,
            rx,
            coms: HashMap::new(),
            connections: HashMap::new(),
            next_uuid: 0,
        }
    }

    pub fn com(&mut self) -> Com {
        let (tx, rx) = mpsc::channel(32);

        let uuid = self.next_uuid;
        self.coms.insert(uuid, tx);
        self.next_uuid += 1;

        Com {
            tx: self.tx.clone(),
            rx,
            uuid,
        }
    }

    pub fn connect(&mut self, u1: &impl HasCom, u2: &impl HasCom) {
        let u1 = u1.com().uuid;
        let u2 = u2.com().uuid;

        self.connections
            .entry(u1)
            .and_modify(|tbl| tbl.push(u2))
            .or_insert(vec![u2]);

        self.connections
            .entry(u2)
            .and_modify(|tbl| tbl.push(u1))
            .or_insert(vec![u1]);
    }

    pub async fn run(mut self) {
        loop {
            if let Some(msg) = self.rx.recv().await {
                if let Some(targets) = self.connections.get(&msg.uuid) {
                    for target_uuid in targets {
                        // msgが送信される時点で送信側はCOMつまりUUIDを持っており、UUIDは初期化時点で明らかにcomsに追加されているので必ずSome
                        let target_com = self.coms.get_mut(target_uuid).unwrap();
                        target_com.send(msg.payload.clone()).await;
                    }
                }
            }
        }
    }
}

pub struct Com {
    tx: Sender<Message>,
    rx: Receiver<Stream>,
    pub uuid: Uuid,
}

impl Com {
    pub async fn send(&mut self, payload: Stream) {
        self.tx
            .send(Message {
                uuid: self.uuid,
                payload,
            })
            .await
            .unwrap();
    }

    pub async fn recv(&mut self) -> Stream {
        self.rx.recv().await.unwrap()
    }
}

#[derive(Debug, Clone)]
pub enum Stream {
    Ipv4,
}

pub trait HasCom {
    fn com(&self) -> &Com;
}
