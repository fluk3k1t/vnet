use std::time::Duration;

use macaddr::MacAddr6;
use tokio::time::sleep;
use vnet::{Core, EthernetCard, EthernetFrame, EthernetFrameType};

#[actix::test]
async fn test_endpoint_connection() {
    let mut core = Core::new();

    let eth0_mac = MacAddr6::new(0x00, 0x00, 0x00, 0x00, 0x00, 0x00);
    let eth0 = EthernetCard::new(core.clone(), eth0_mac, false);

    let eth1_mac = MacAddr6::new(0x00, 0x00, 0x00, 0x00, 0x00, 0x01);
    let eth1 = EthernetCard::new(core.clone(), eth1_mac, false);

    let eth2_mac = MacAddr6::new(0x00, 0x00, 0x00, 0x00, 0x00, 0x02);
    let eth2 = EthernetCard::new(core.clone(), eth2_mac, false);

    core.connect(&eth0, &eth1).await;

    eth0.send(eth1_mac, EthernetFrameType::Dummy);

    sleep(Duration::from_millis(10)).await;

    let ef = eth1.recv().await;
    assert_eq!(
        ef,
        Some(EthernetFrame::new(
            eth0_mac,
            eth1_mac,
            EthernetFrameType::Dummy
        ))
    );

    let ef = eth2.recv().await;
    assert_eq!(ef, None);
}
