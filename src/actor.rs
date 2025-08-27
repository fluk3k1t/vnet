use std::marker::PhantomData;
use tokio::{
    sync::{
        Mutex,
        mpsc::{self, UnboundedReceiver, UnboundedSender},
        oneshot::{self, Sender},
    },
    task::JoinHandle,
};

pub trait Actor {
    type Context: Sized + Send;

    fn start(mut self) -> (Caller<Self>, JoinHandle<()>)
    where
        Self: Sized + Send + 'static,
    {
        let (caller, mut mb) = Mailbox::<Self>::new();

        let join = tokio::spawn(async move {
            loop {
                let env = mb.recv().await;
                let ret = env.handle(&mut self);
            }

            ()
        });

        (caller, join)
    }
}

pub trait Handler<M>
where
    M: Message,
{
    fn handle(&mut self, m: M) -> M::Return;
}

pub trait EnvelopeProxy<A>: Send {
    fn handle(self: Box<Self>, a: &mut A);
}

pub struct Envelope<A, M>
where
    M: Message,
{
    m: M,
    s: Sender<M::Return>,
    _a: PhantomData<A>,
}

impl<A, M> Envelope<A, M>
where
    M: Message,
{
    pub fn new(m: M, s: Sender<M::Return>) -> Self {
        Envelope {
            m,
            _a: PhantomData,
            s,
        }
    }
}

impl<A, M> EnvelopeProxy<A> for Envelope<A, M>
where
    A: Handler<M> + Send,
    M: Message + Send,
{
    fn handle(self: Box<Self>, a: &mut A) {
        let ret = a.handle(self.m);
        self.s.send(ret);
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
    pub async fn call<M>(&mut self, m: M) -> M::Return
    where
        A: Handler<M> + Send + 'static,
        M: Message + Send + 'static,
    {
        let (s, r) = oneshot::channel();
        let env = Envelope::new(m, s);

        self.s.send(Box::new(env)).unwrap();

        r.await.unwrap()
    }
}

pub trait Message {
    type Return: Send;
}
