use anyhow::{Context, Result, ensure};
use std::collections::{HashMap, VecDeque};
use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::EthernetFrame;

pub type Uuid = u32;

#[derive(Debug)]
pub struct Message {
    uuid: Uuid,
    payload: Pdu,
}

pub enum Command {
    Shutdown,
}

pub struct Core {
    rx: Receiver<Message>,
    tx: Sender<Message>,
    syshandler: Receiver<Command>,
    syscaller: Sender<Command>,
    coms: HashMap<Uuid, Sender<Pdu>>,
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
            buffer: VecDeque::new(),
            syscaller: self.syscaller.clone(),
            uuid,
        }
    }

    pub fn connect(&mut self, u1: &impl HasUuid, u2: &impl HasUuid) {
        let u1 = u1.uuid();
        let u2 = u2.uuid();

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
                                if let Ok(_) = target_com.send(msg.payload.clone()).await {
                                } else {
                                    // Errをはじくなら、Comが所有者がrunしないタイプでもdropされないようにしなければならない
                                    // 宛先が存在しない、というのは論理エラーとすることもできるが、処理しないにしても何らかのログや警告の通知を行いたい
                                    println!("target com port {:?} is already closed!", target_com);
                                }
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

        println!("Core main loop has ended");
    }
}

// syscallerとmessageを分ける必要あるのか？？？
pub struct Com {
    tx: Sender<Message>,
    rx: Receiver<Pdu>,
    syscaller: Sender<Command>,
    pub uuid: Uuid,
    buffer: VecDeque<Pdu>,
}

// Coreのメインループが回る前にcallしたりするとchannelが開かれていないので必ずエラーになってしまう、、、
impl Com {
    pub async fn send(&self, payload: Pdu) {
        if let Err(err) = self
            .tx
            .send(Message {
                uuid: self.uuid,
                payload,
            })
            .await
        {
            println!("{:?}", err);
        }
    }

    pub fn received(&mut self) -> bool {
        if !self.buffer.is_empty() {
            return true;
        }
        match self.rx.try_recv() {
            Ok(msg) => {
                self.buffer.push_back(msg);
                true
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => false,
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => false,
        }
    }

    pub async fn recv(&mut self) -> Pdu {
        if let Some(msg) = self.buffer.pop_front() {
            return msg;
        }

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
pub enum Pdu {
    Ipv4,
    EthernetFrame(EthernetFrame<String>),
    Dummy,
}

pub trait HasUuid {
    fn uuid(&self) -> Uuid;
}
