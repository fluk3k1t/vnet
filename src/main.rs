use std::{net::Ipv4Addr, time::Duration};

use const_addrs::{ip4, mac6};
use tokio::{join, time::sleep};
use tracing::Level;
use vnet::{Core, IPv4PacketType, L2Sw, NetworkInterfaceCard, l2};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_level(true)
        .with_max_level(Level::TRACE)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NEW)
        .with_target(true)
        .init();

    let core = Core::spawn().await;

    let nic0 = NetworkInterfaceCard::spawn(
        core.clone(),
        ip4!("192.168.0.1"),
        mac6!("00:00:00:00:00:01"),
    )
    .await;

    let nic1 = NetworkInterfaceCard::spawn(
        core.clone(),
        ip4!("192.168.0.2"),
        mac6!("00:00:00:00:00:02"),
    )
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
