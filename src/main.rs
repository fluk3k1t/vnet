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
            loop {
                let r: Self::Message = r.recv().await.unwrap();

                self.handle(r, &mut ctx);
            }
        });

        s
    }
}

pub trait DefaultValue {
    fn default_value() -> Self;
}

pub trait Handler<M, C> {
    fn handle(&self, m: M, ctx: &mut C);
}

pub struct Caller {}

pub struct MrJohn;

pub struct MrJohnSt {}

pub enum Inst {}

impl Handler<Inst, MrJohnSt> for MrJohn {
    fn handle(&self, m: Inst, ctx: &mut MrJohnSt) {}
}

impl DefaultValue for MrJohnSt {
    fn default_value() -> Self {
        MrJohnSt {}
    }
}

impl Reactor for MrJohn {
    type Context = MrJohnSt;
    type Message = Inst;
}

fn main() {
    // let mut mrjohn = MrJohn {};
    // mrjohn.start();
    MrJohn.start();
}
