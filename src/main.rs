use once_cell::sync::Lazy;
use std::{marker::PhantomData, sync::Arc};
use tokio::sync::{
    Mutex,
    mpsc::{self, UnboundedReceiver, UnboundedSender},
};

pub struct Core {}

static CORE: Lazy<Arc<Mutex<Core>>> = Lazy::new(|| Arc::new(Mutex::new(Core {})));

pub trait Reactor {
    type Message: Send + 'static;
    type Context: Send + 'static + DefaultValue;

    fn start(self) -> UnboundedSender<Self::Message>
    where
        Self: Send + 'static + Sized + Handler<Self::Message, Self::Context>,
    {
        let (s, mut r) = mpsc::unbounded_channel();
        let mut ctx: Self::Context = Self::Context::default_value();

        tokio::spawn(async move {
            while let Some(r) = r.recv().await {
                self.handle(r, &mut ctx).await;
            }
        });

        s
    }
}

pub trait DefaultValue {
    fn default_value() -> Self;
}

pub trait Handler<M, C> {
    fn handle(&mut self, m: M, ctx: &mut C) -> impl std::future::Future<Output = ()> + Send;
}

pub struct Caller {}

pub struct MrJohn;

pub struct MrJohnSt {
    age: usize,
}

#[derive(Debug)]
pub enum Inst {
    Dm,
}

impl Handler<Inst, MrJohnSt> for MrJohn {
    async fn handle(&mut self, m: Inst, ctx: &mut MrJohnSt) {
        println!("handle {:?}", m);
    }
}

impl DefaultValue for MrJohnSt {
    fn default_value() -> Self {
        MrJohnSt { age: 0 }
    }
}

impl Reactor for MrJohn {
    type Context = MrJohnSt;
    type Message = Inst;
}

#[tokio::main]
async fn main() {
    // let mut mrjohn = MrJohn {};
    // mrjohn.start();
    let mut mrjohn = MrJohn.start();
    mrjohn.send(Inst::Dm).unwrap();
}
