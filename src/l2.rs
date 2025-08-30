use std::{collections::VecDeque, iter::repeat_with, ops::Index, sync::Arc};

use actix::prelude::*;
use futures::future::join_all;
use macaddr::MacAddr6;
use tokio::sync::Mutex;
use tracing::debug;

use crate::{Connectable, Core, EndPoint, EthernetFrame, EthernetFrameType, OnReceive, Uuid};

pub struct L2Sw {
    addr: Addr<L2SwRaw>,
}

impl L2Sw {
    pub fn new(core: Core, n_ports: usize) -> Self {
        L2Sw {
            addr: L2SwRaw::new(core, n_ports).start(),
        }
    }

    pub async fn port(&self, n_port: usize) -> Option<Port> {
        self.addr.send(GetPort(n_port)).await.unwrap()
    }
}

struct L2SwRaw {
    ports: Arc<Mutex<Option<Vec<Port>>>>,
    core: Core,
    n_ports: usize,
}

impl L2SwRaw {
    fn new(core: Core, n_ports: usize) -> Self {
        L2SwRaw {
            ports: Arc::new(Mutex::new(None)),
            core,
            n_ports,
        }
    }
}

impl Actor for L2SwRaw {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let me = ctx.address();
        let on_receive = me.clone().recipient();
        let core = self.core.clone();
        let n_ports = self.n_ports;
        let ports_arc = self.ports.clone();

        ctx.wait(
            async move {
                let ports = {
                    let mut tmp = vec![];
                    for _ in 0..n_ports {
                        tmp.push(Port::new(core.create_ep(on_receive.clone()).await));
                    }
                    tmp
                };

                ports_arc.lock().await.replace(ports);
            }
            .into_actor(self),
        );
    }
}

#[derive(Clone, Debug)]
pub struct Port {
    ep: EndPoint,
}

impl Port {
    pub fn new(ep: EndPoint) -> Self {
        Port { ep }
    }

    pub async fn uuid(&self) -> Uuid {
        self.ep.uuid().await
    }
}

impl Connectable for Port {
    async fn uuid(&self) -> Uuid {
        self.ep.uuid().await
    }
}

// 同じアクターでもメッセージレベルで並列だからハンドラごとにFutureが必要
// そしてユーザーはそんなものを求めていなくて、事実ステートをArcにする煩雑さが生じている
// というか俺のケースだとメッセージひとつひとつが時間のかかる処理ではないのでメッセージが並列である意味がなく、Arcのメンドサだけが強調されてしまう
// シミュレーション用のオブジェクト指向はactor modelより良いものがありそう
// それもあるし、一番の原因はstartedとかでasyncな処理をする必要（create_epなど）があって、それでselfを更新するにはarcが必要だから
// でendpointのようなアクターにするほどでもない通信路をアクターにしてしまっているから、create_epが非同期になって？？？
// いや関係ないか、coreにコールする時点でasyncにはなる、、、
// startedに初期化を委譲しなくてもいいようにendpointを設計しよう
// いやon receive形式な時点でactorが起動するまでreceipientを取得できないのでそんなものは不可能
// listen(endpoint)的なものがあれば理想的
// Uuidみたいなメンバー変数をゲットするためにいちいちgetterを書いてるのオブジェクト指向の典型的な失敗と呼ばれているやつぅ
impl Handler<OnReceive> for L2SwRaw {
    type Result = ResponseFuture<()>;

    fn handle(&mut self, msg: OnReceive, ctx: &mut Self::Context) -> Self::Result {
        let ports = self.ports.clone();
        Box::pin(async move {
            for port in ports.lock().await.as_mut().unwrap().iter_mut() {
                // msg.dst == 受け取ったポートのUuid
                // 受信したポート以外のポートから送信する
                if port.uuid().await != msg.dst {
                    port.ep.send(msg.payload.clone());
                }
            }
        })
    }
}

#[derive(Message)]
#[rtype(result = "Option<Port>")]
struct GetPort(usize);

impl Handler<GetPort> for L2SwRaw {
    type Result = ResponseFuture<Option<Port>>;

    fn handle(&mut self, msg: GetPort, ctx: &mut Self::Context) -> Self::Result {
        let ports = self.ports.clone();

        Box::pin(async move {
            let mut ports = ports.lock().await;
            let ports = ports.as_mut()?;
            let port = ports.get(msg.0)?;
            Some(port.clone())
        })
    }
}

#[derive(Clone)]
pub struct EthernetCard {
    addr: Addr<EthernetCardRaw>,
}

impl EthernetCard {
    pub fn new(core: Core, mac: MacAddr6, is_promiscuous: bool) -> Self {
        EthernetCard {
            addr: EthernetCardRaw::new(core, mac, is_promiscuous).start(),
        }
    }

    pub fn send(&self, dst: MacAddr6, payload: EthernetFrameType) {
        self.addr.do_send(EthernetCardRawSend { dst, payload });
    }

    pub async fn recv(&self) -> Option<EthernetFrame> {
        self.addr.send(EthernetCardRawRecv {}).await.unwrap()
    }

    pub async fn uuid(&self) -> Uuid {
        self.addr.send(EthernetCardRawGetUuid {}).await.unwrap()
    }
}

impl Connectable for EthernetCard {
    async fn uuid(&self) -> Uuid {
        self.addr.send(EthernetCardRawGetUuid {}).await.unwrap()
    }
}

struct EthernetCardRaw {
    mac: MacAddr6,
    ep: Arc<Mutex<Option<EndPoint>>>,
    core: Core,
    rx_buffer: VecDeque<EthernetFrame>,
    is_promiscuous: bool,
}

impl EthernetCardRaw {
    fn new(core: Core, mac: MacAddr6, is_promiscuous: bool) -> Self {
        EthernetCardRaw {
            ep: Arc::new(Mutex::new(None)),
            mac,
            core,
            rx_buffer: VecDeque::new(),
            is_promiscuous,
        }
    }
}

impl Actor for EthernetCardRaw {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let ep = self.ep.clone();
        let core = self.core.clone();
        let on_receive = ctx.address().recipient();

        ctx.wait(
            async move {
                let created_ep = core.create_ep(on_receive).await;
                let _ = ep.lock().await.replace(created_ep);
            }
            .into_actor(self),
        );
    }
}

impl Handler<OnReceive> for EthernetCardRaw {
    type Result = ();

    fn handle(&mut self, msg: OnReceive, ctx: &mut Self::Context) -> Self::Result {
        debug!("ethernet card on receive");
        self.rx_buffer.push_back(msg.payload);
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct EthernetCardRawSend {
    dst: MacAddr6,
    payload: EthernetFrameType,
}

impl Handler<EthernetCardRawSend> for EthernetCardRaw {
    type Result = ResponseFuture<()>;

    fn handle(&mut self, msg: EthernetCardRawSend, ctx: &mut Self::Context) -> Self::Result {
        let ef = EthernetFrame::new(self.mac, msg.dst, msg.payload);
        let ep = self.ep.clone();

        Box::pin(async move {
            if let Some(ep) = ep.lock().await.as_mut() {
                ep.send(ef);
            }
        })
    }
}

#[derive(Message)]
#[rtype(result = "Option<EthernetFrame>")]
struct EthernetCardRawRecv {}

impl Handler<EthernetCardRawRecv> for EthernetCardRaw {
    type Result = Option<EthernetFrame>;

    fn handle(&mut self, msg: EthernetCardRawRecv, ctx: &mut Self::Context) -> Self::Result {
        let ef = self.rx_buffer.pop_front()?;

        if ef.dst == self.mac || self.is_promiscuous || ef.dst.is_broadcast() {
            return Some(ef);
        }

        None
    }
}

#[derive(Message)]
#[rtype(result = "Uuid")]
struct EthernetCardRawGetUuid {}

impl Handler<EthernetCardRawGetUuid> for EthernetCardRaw {
    type Result = ResponseFuture<Uuid>;

    fn handle(&mut self, msg: EthernetCardRawGetUuid, ctx: &mut Self::Context) -> Self::Result {
        let ep = self.ep.clone();

        Box::pin(async move {
            let mut ep = ep.lock().await;
            let ep = ep.as_mut().unwrap();
            ep.uuid().await
        })
    }
}
