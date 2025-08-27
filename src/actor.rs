use once_cell::sync::Lazy;
use std::{marker::PhantomData, sync::Arc, thread::sleep, time::Duration};
use tokio::{
    sync::{
        Mutex,
        mpsc::{self, UnboundedReceiver, UnboundedSender},
    },
    task::JoinHandle,
};

pub trait Actor {
    type Context: Sized + Send;

    fn start(mut self) -> Caller<Self>
    where
        Self: Sized + Send + 'static,
    {
        let (caller, mut mb) = Mailbox::<Self>::new();

        tokio::spawn(async move {
            loop {
                let r = mb.recv().await;
                r.handle(&mut self);
            }
        });

        caller
    }
}

pub trait Handler<M> {
    fn handle(&mut self, m: M);
}

pub trait EnvelopeProxy<A>: Send {
    fn handle(self: Box<Self>, a: &mut A);
}

pub struct Envelope<A, M> {
    m: M,
    _a: PhantomData<A>,
}

impl<A, M> Envelope<A, M> {
    pub fn new(m: M) -> Self {
        Envelope { m, _a: PhantomData }
    }
}

impl<A, M> EnvelopeProxy<A> for Envelope<A, M>
where
    A: Handler<M> + Send,
    M: Send,
{
    fn handle(self: Box<Self>, a: &mut A) {
        a.handle(self.m);
    }
}

pub struct Mailbox<A: Actor> {
    r: UnboundedReceiver<Box<dyn EnvelopeProxy<A>>>,
}

pub struct Caller<A: Actor> {
    s: UnboundedSender<Box<dyn EnvelopeProxy<A>>>,
}

impl<A: Actor> Mailbox<A> {
    pub fn new() -> (Caller<A>, Self) {
        let (s, r) = mpsc::unbounded_channel();

        (Caller { s }, Mailbox { r })
    }

    pub async fn recv(&mut self) -> Box<dyn EnvelopeProxy<A>> {
        self.r.recv().await.unwrap()
    }
}

impl<A: Actor> Caller<A> {
    pub fn send<M>(&mut self, m: M)
    where
        A: Handler<M> + Send + 'static,
        M: Send + 'static,
    {
        let env = Envelope::new(m);
        self.s.send(Box::new(env)).unwrap();
    }
}
