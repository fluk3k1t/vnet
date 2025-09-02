use const_addrs::{ip4, mac6};
use std::{net::Ipv4Addr, time::Duration};
use tokio::{join, time::sleep};
use tracing::{Level, Subscriber, info_span, level_filters::LevelFilter};
use tracing_subscriber::{
    Layer,
    filter::{self, Targets, dynamic_filter_fn, filter_fn},
    fmt::format::FmtSpan,
    layer::SubscriberExt,
    util::SubscriberInitExt,
};
use vnet::{
    Core, DefaultGateway, EthernetCard, IPv4PacketType, L2Sw, L3Sw, L3SwBuilder,
    NetworkInterfaceCard, NetworkInterfaceCardBuilder, l2,
};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .finish()
        .with(dynamic_filter_fn(|meta, cx| true))
        .init();

    let core = Core::spawn().await;

    let nic1 = NetworkInterfaceCardBuilder::new()
        .core(core.clone())
        .ip(ip4!("192.168.1.1"))
        .mac(mac6!("00:00:00:00:00:01"))
        .default_gateway(DefaultGateway::new(
            ip4!("192.168.1.255"),
            ip4!("255.255.255.0"),
        ))
        .tag("nic1")
        .spawn()
        .await;

    let nic2 = NetworkInterfaceCardBuilder::new()
        .core(core.clone())
        .ip(ip4!("192.168.2.1"))
        .mac(mac6!("00:00:00:00:00:02"))
        .default_gateway(DefaultGateway::new(
            ip4!("192.168.2.255"),
            ip4!("255.255.255.0"),
        ))
        .tag("nic2")
        .spawn()
        .await;

    let l3sw = L3SwBuilder::new(core.clone())
        .port(ip4!("192.168.1.255"), ip4!("255.255.255.0"))
        .port(ip4!("192.168.2.255"), ip4!("255.255.255.0"))
        .spawn()
        .await;

    // core.connect
}
