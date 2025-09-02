use const_addrs::{ip4, mac6};
use std::{net::Ipv4Addr, time::Duration};
use tokio::{join, time::sleep};
use tracing::{Level, Subscriber, info_span, level_filters::LevelFilter};
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

#[tokio::main]
async fn main() {
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
