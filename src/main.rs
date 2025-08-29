use std::time::Duration;

use actix::prelude::*;
use macaddr::MacAddr6;
use tokio::{join, time::sleep};
use tracing::{Level, debug};
use vnet::{Core, EthernetCard, EthernetFrameType, L2Sw};

#[actix::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_file(true)
        .with_line_number(true)
        .with_thread_names(true)
        .with_level(true)
        .with_max_level(Level::DEBUG)
        .init();

    let mut core = Core::new();

    let eth0 = EthernetCard::new(
        core.clone(),
        MacAddr6::new(0x00, 0x00, 0x00, 0x00, 0x00, 0x00),
    );

    let eth1 = EthernetCard::new(
        core.clone(),
        MacAddr6::new(0x00, 0x00, 0x00, 0x00, 0x00, 0x01),
    );

    let l2sw = L2Sw::new(core.clone(), 2);

    core.connect(&eth0, &l2sw.port(0).await.unwrap()).await;
    core.connect(&eth1, &l2sw.port(1).await.unwrap()).await;

    let j1 = tokio::spawn(async move {
        eth0.send(
            MacAddr6::new(0x00, 0x00, 0x00, 0x00, 0x00, 0x01),
            EthernetFrameType::Dummy,
        );
    });

    let j2 = tokio::spawn(async move {
        sleep(Duration::from_secs(1)).await;
        let r = eth1.recv().await;
        println!("{:?}", r);
    });

    join!(j2);
}
