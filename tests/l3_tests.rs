use std::{net::Ipv4Addr, time::Duration};

use macaddr::MacAddr6;
use tokio::time::sleep;
use vnet::{
    Core, EthernetCard, EthernetFrame, EthernetFrameType, IPv4PacketType, L2Sw, NetworkCard,
    NetworkDriver,
};

#[actix::test]
async fn test_l3_arp() {
    let core = Core::new();

    let nic0 = NetworkCard::new(
        core.clone(),
        Ipv4Addr::new(192, 168, 0, 0),
        MacAddr6::new(0x00, 0x00, 0x00, 0x00, 0x00, 0x00),
    )
    .build();
    let nth0 = NetworkDriver::new(nic0);

    let nic1 = NetworkCard::new(
        core.clone(),
        Ipv4Addr::new(192, 168, 0, 1),
        MacAddr6::new(0x00, 0x00, 0x00, 0x00, 0x00, 0x01),
    )
    .build();
    let nth1 = NetworkDriver::new(nic1);

    let l2sw = L2Sw::new(core.clone(), 2);

    core.connect(&nth0, &l2sw.port(0).await.unwrap()).await;
    core.connect(&nth1, &l2sw.port(1).await.unwrap()).await;

    nth0.send(
        Ipv4Addr::new(192, 168, 0, 1),
        IPv4PacketType::Debug("dummy".to_string()),
    );

    sleep(Duration::from_millis(10)).await;

    let r = nth1.recv().await;
    assert_eq!(r, None);
}
