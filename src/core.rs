use anyhow::{Context, Result, ensure};
use std::collections::HashMap;
use tokio::sync::mpsc::{self, Receiver, Sender};

pub type Uuid = u32;

pub struct Message {
    uuid: Uuid,
    payload: Stream,
}

pub enum Command {
    Shutdown,
}

pub struct Core {
    rx: Receiver<Message>,
    tx: Sender<Message>,
    syshandler: Receiver<Command>,
    syscaller: Sender<Command>,
    coms: HashMap<Uuid, Sender<Stream>>,
    connections: HashMap<Uuid, Vec<Uuid>>,
    next_uuid: Uuid,
}

impl Core {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(32);
        let (syscaller, syshandler) = mpsc::channel(32);

        Core {
            tx,
            rx,
            syshandler,
            syscaller,
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
            syscaller: self.syscaller.clone(),
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
            tokio::select! {
                msg = self.rx.recv() => {
                    if let Some(msg) = msg {
                        if let Some(targets) = self.connections.get(&msg.uuid) {
                            for target_uuid in targets {
                                // msgが送信される時点で送信側はCOMつまりUUIDを持っており、UUIDは初期化時点で明らかにcomsに追加されているので必ずSome
                                let target_com = self.coms.get_mut(target_uuid).unwrap();
                                target_com.send(msg.payload.clone()).await.unwrap();
                            }
                        } else {
                            println!("unconnected!");
                        }
                    } else {
                        println!("all senders were dropped!");
                        break;
                    }
                }

                cmd = self.syshandler.recv() => {
                    match cmd.unwrap() {
                        Command::Shutdown => break,
                        _ => unreachable!(),
                    }
                }
            }
        }
    }
}

// syscallerとmessageを分ける必要あるのか？？？
pub struct Com {
    tx: Sender<Message>,
    rx: Receiver<Stream>,
    syscaller: Sender<Command>,
    pub uuid: Uuid,
}

// Coreのメインループが回る前にcallしたりするとchannelが開かれていないので必ずエラーになってしまう、、、
impl Com {
    pub async fn send(&self, payload: Stream) {
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

    pub async fn call(&mut self, cmd: Command) {
        self.syscaller.send(cmd).await.unwrap();
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        // ShutdownなりすべてのComが終了したのちメインループを終了させるようなコマンドを送出
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stream {
    Ipv4,
    Dummy,
}

pub trait HasCom {
    fn com(&self) -> &Com;
}
