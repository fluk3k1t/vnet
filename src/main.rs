use once_cell::sync::Lazy;
use std::{marker::PhantomData, sync::Arc, thread::sleep, time::Duration};
use tokio::sync::{
    Mutex,
    mpsc::{self, UnboundedReceiver, UnboundedSender},
};

pub trait Actor<T> {
    type Context;

    fn start(mut self) -> Caller<T>
    where
        Self: Handler<T> + Send + Sized + 'static,
        T: Send + 'static,
    {
        // let mut ctx = Context::new(self);
        let (mut mb, caller) = Mailbox::new();

        tokio::spawn(async move {
            loop {
                let r = mb.recv().await;
                self.handle(r);
            }
        });

        caller
    }
}

pub trait Handler<M> {
    fn handle(&mut self, m: M);
}

pub struct Mailbox<M> {
    r: UnboundedReceiver<M>,
}

impl<M> Mailbox<M> {
    pub fn new() -> (Self, Caller<M>) {
        let (s, r) = mpsc::unbounded_channel();

        (Mailbox { r }, Caller::new(s))
    }

    pub async fn recv(&mut self) -> M {
        self.r.recv().await.unwrap()
    }
}

pub struct Caller<M> {
    s: UnboundedSender<M>,
}

impl<M> Caller<M> {
    pub fn new(s: UnboundedSender<M>) -> Self {
        Caller { s }
    }

    pub fn call(&mut self, m: M) {
        self.s.send(m).unwrap();
    }
}

pub struct Context<T> {
    st: T,
}

impl<T> Context<T> {
    pub fn new(st: T) -> Self {
        Context { st }
    }
}

pub struct John {
    pub age: usize,
}

pub enum JohnInst {
    Say(String),
}

impl Handler<JohnInst> for John {
    fn handle(&mut self, m: JohnInst) {
        println!("handle");

        match m {
            JohnInst::Say(c) => println!("say {}", c),
        }
    }
}

impl Actor<JohnInst> for John {
    type Context = John;
}

#[tokio::main]
async fn main() {
    let mut john = John { age: 10 };
    let mut john = john.start();

    john.call(JohnInst::Say("hello, my name is john".to_string()));

    sleep(Duration::from_secs(1));
}
