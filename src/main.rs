use const_addrs::{ip4, mac6};
use std::{net::Ipv4Addr, time::Duration};
use tokio::{join, time::sleep};
use tracing::{Level, Subscriber, level_filters::LevelFilter, info_span};
// use tracing_subscriber::EnvFilter;
use tracing_subscriber::{
    Layer,
    filter::{self, Targets, filter_fn},
    fmt::format::FmtSpan,
    layer::SubscriberExt,
    util::SubscriberInitExt,
};
use vnet::{
    Core, EthernetCard, IPv4PacketType, L2Sw, NetworkInterfaceCard, NetworkInterfaceCardBuilder, l2,
};

pub fn type_of<T>(_: &T) -> &'static str {
    std::any::type_name::<T>()
}

struct SpanTargetFilterLayer;

impl<S> tracing_subscriber::Layer<S> for SpanTargetFilterLayer
where
    S: tracing::Subscriber,
    for<'a> S: tracing_subscriber::registry::LookupSpan<'a>,
{
    fn enabled(
        &self,
        metadata: &tracing::Metadata<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        if let Some(span) = ctx.lookup_current() {
            let target = span.metadata().target();
            println!("fffffffffffffffffffffffffffffffffffffffffff {}", target);
            return target == "ethernetcard";
        }
        // デフォルト: 出さない
        false
    }
}

#[tokio::main]
async fn main() {
    // let filter = EnvFilter::from_default_env();
    let f = tracing_subscriber::fmt::layer()
        // .with_span_events(FmtSpan::ENTER)
        // .with_span_events(FmtSpan::F)
        .with_filter(LevelFilter::INFO)
        .with_filter(filter_fn(|metadata| {
            let r = metadata.is_event();

            if r {
                let p = tracing_subscriber::registry::Registry::current_span(
                    &tracing_subscriber::registry(),
                );
                // let q = tracing_subscriber::FmtSubscriber::current_span(
                //     &tracing_subscriber::f,
                // );
                // println!("{:?} {:?}", p, metadata);
            }

            false
        }));

    // let f = tracing_subscriber::fmt()
    //     .with_env_filter(EnvFilter::new("ethernetcard=info"))
    //     .init();

    tracing_subscriber::registry().with(f).init();
    let span = info_span!("span_1", key = "hello");
    let _guard = span.enter();
    println!("{:?}", tracing_subscriber::registry());

    let core = Core::spawn().await;

    let nic0 = NetworkInterfaceCardBuilder::new()
        .core(core.clone())
        .ip(ip4!("192.168.0.1"))
        .mac(mac6!("00:00:00:00:00:01"))
        .tag("nic0")
        .spawn()
        .await;

    let nic1 = NetworkInterfaceCardBuilder::new()
        .core(core.clone())
        .ip(ip4!("192.168.0.2"))
        .mac(mac6!("00:00:00:00:00:02"))
        .tag("nic1")
        .spawn()
        .await;

    let l2sw = L2Sw::spawn(core.clone(), 2).await;

    core.connect(nic0.uuid().await, l2sw.port(0).await.unwrap().uuid().await)
        .await;

    core.connect(nic1.uuid().await, l2sw.port(1).await.unwrap().uuid().await)
        .await;

    tokio::spawn(async move {
        nic0.send(
            ip4!("192.168.0.2"),
            IPv4PacketType::Debug("dummy".to_string()),
        );
    });

    let j = tokio::spawn(async move {
        let r = nic1.recv_blocking().await;
        println!("ttttttttttttttttttttttttttt {:?}", r);
    });

    j.await;
}
