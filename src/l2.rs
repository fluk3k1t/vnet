use std::{iter::repeat_with, ops::Index, sync::Arc};

use actix::prelude::*;
use futures::future::join_all;
use tokio::sync::Mutex;

use crate::{Core, EndPoint, EthernetFrame, OnReceive, Uuid};

pub struct L2Sw {
    addr: Addr<L2SwRaw>,
}

impl L2Sw {
    pub fn new(core: Core, n_ports: usize) -> Self {
        L2Sw {
            addr: L2SwRaw::new(core, n_ports).start(),
        }
    }
}

pub struct L2SwRaw {
    ports: Arc<Mutex<Option<Vec<EndPoint>>>>,
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

        ctx.spawn(
            async move {
                let ports = {
                    let mut tmp = vec![];
                    for _ in 0..n_ports {
                        tmp.push(core.create_ep(on_receive.clone()).await);
                    }
                    tmp
                };

                me.send(SetPorts(ports)).await.unwrap();
            }
            .into_actor(self),
        );
    }
}

impl Handler<OnReceive> for L2SwRaw {
    type Result = ();
    fn handle(&mut self, msg: OnReceive, ctx: &mut Self::Context) -> Self::Result {}
}

#[derive(Message)]
#[rtype(result = "Option<Uuid>")]
struct GetPortUuid(usize);

impl Handler<GetPortUuid> for L2SwRaw {
    type Result = ResponseFuture<Option<Uuid>>;

    fn handle(&mut self, msg: GetPortUuid, ctx: &mut Self::Context) -> Self::Result {
        let ports = self.ports.clone();

        Box::pin(async move {
            let mut ports = ports.lock().await;
            let ports = ports.as_mut()?;
            let port = ports.get(msg.0)?;
            Some(port.uuid().await)
        })
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct SetPorts(Vec<EndPoint>);

impl Handler<SetPorts> for L2SwRaw {
    type Result = ResponseFuture<()>;
    fn handle(&mut self, msg: SetPorts, ctx: &mut Self::Context) -> Self::Result {
        let ports = self.ports.clone();
        Box::pin(async move {
            let _ = ports.lock().await.replace(msg.0);
        })
    }
}
