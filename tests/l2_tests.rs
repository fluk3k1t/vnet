use std::time::Duration;

use macaddr::MacAddr6;
use tokio::time::sleep;
use vnet::{Core, EthernetCard, EthernetFrame, EthernetFrameType, L2Sw};

#[actix::test]
async fn test_l2_connected() {
    let core = Core::new();

    let eth0_mac = MacAddr6::new(0x00, 0x00, 0x00, 0x00, 0x00, 0x00);
    let eth0 = EthernetCard::new(core.clone(), eth0_mac);

    let eth1_mac = MacAddr6::new(0x00, 0x00, 0x00, 0x00, 0x00, 0x01);
    let eth1 = EthernetCard::new(core.clone(), eth1_mac);

    let eth2_mac = MacAddr6::new(0x00, 0x00, 0x00, 0x00, 0x00, 0x02);
    let eth2 = EthernetCard::new(core.clone(), eth2_mac);

    let l2sw = L2Sw::new(core.clone(), 3);

    core.connect(&eth0, &l2sw.port(0).await.unwrap()).await;
    core.connect(&eth1, &l2sw.port(1).await.unwrap()).await;
    core.connect(&eth1, &l2sw.port(2).await.unwrap()).await;

    eth0.send(
        MacAddr6::new(0x00, 0x00, 0x00, 0x00, 0x00, 0x01),
        EthernetFrameType::Dummy,
    );

    sleep(Duration::from_millis(10)).await;

    let r = eth1.recv().await;
    assert_eq!(
        r,
        Some(EthernetFrame::new(
            eth0_mac,
            eth1_mac,
            EthernetFrameType::Dummy
        ))
    );

    let r = eth2.recv().await;
    assert_eq!(r, None);
}
