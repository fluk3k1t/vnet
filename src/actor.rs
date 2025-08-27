use std::{fmt::Debug, marker::PhantomData};
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
        let mut ctx = Context::new(caller.clone());

        let join = tokio::spawn(async move {
            while let Some(env) = mb.recv().await {
                env.handle(&mut self, &mut ctx);
            }

            ()
        });

        (caller, join)
    }
}

pub trait Handler<M>
where
    Self: Actor + Sized,
    M: Message,
{
    fn handle(&mut self, m: M, ctx: &mut Context<Self>) -> M::Return;
}

pub trait EnvelopeProxy<A>: Send
where
    A: Actor,
{
    fn handle(self: Box<Self>, a: &mut A, ctx: &mut Context<A>);
}

#[derive(Debug)]
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
            s,
            _a: PhantomData,
        }
    }
}

impl<A, M> EnvelopeProxy<A> for Envelope<A, M>
where
    A: Actor + Handler<M> + Send,
    M: Message + Send,
{
    fn handle(self: Box<Self>, a: &mut A, ctx: &mut Context<A>) {
        let ret = a.handle(self.m, ctx);
        self.s.send(ret).expect("EnvelopeProxy: return failed");
    }
}

pub struct Mailbox<A: Actor> {
    r: UnboundedReceiver<Box<dyn EnvelopeProxy<A>>>,
}

#[derive(Debug)]
pub struct Caller<A: Actor> {
    s: UnboundedSender<Box<dyn EnvelopeProxy<A>>>,
}

impl<A: Actor> Mailbox<A> {
    pub fn new() -> (Caller<A>, Self) {
        let (s, r) = mpsc::unbounded_channel();

        (Caller { s }, Mailbox { r })
    }

    pub async fn recv(&mut self) -> Option<Box<dyn EnvelopeProxy<A>>> {
        self.r.recv().await
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

    pub fn clone(&self) -> Self {
        Caller { s: self.s.clone() }
    }
}

pub trait Message {
    type Return: Send + Debug;
}

pub struct Context<A>
where
    A: Actor,
{
    caller: Caller<A>,
}

impl<A> Context<A>
where
    A: Actor,
{
    pub fn new(caller: Caller<A>) -> Self {
        Context { caller }
    }

    pub fn caller(&self) -> Caller<A> {
        self.caller.clone()
    }
}
