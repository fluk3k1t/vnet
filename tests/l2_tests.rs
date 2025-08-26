use std::{thread::sleep, time::Duration};

use macaddr::MacAddr6;
use tokio::test;
use vnet::{Command, Core, Device, EthernetFrame, EthernetFrameType, L2Sw, Pdu};

#[tokio::test]
async fn test_l2_learning() {
    let mut core = Core::new();

    let mut sw0 = L2Sw::new(&mut core, 3);
    let mut d0 = Device::new(&mut core);
    let mut d1 = Device::new(&mut core);
    let mut d2 = Device::new(&mut core);

    let d0mac = MacAddr6::new(0x00, 0x00, 0x00, 0x00, 0x00, 0x00);
    let d1mac = MacAddr6::new(0x00, 0x00, 0x00, 0x00, 0x00, 0x01);

    core.connect(&sw0.com(0), &d0);
    core.connect(&sw0.com(1), &d1);
    core.connect(&sw0.com(2), &d2);

    sw0.run();

    let msg0 = EthernetFrame::new(
        d0mac,
        d1mac,
        EthernetFrameType::Debug("from d0 before learning".to_string()),
    );

    let msg1 = EthernetFrame::new(
        d1mac,
        d0mac,
        EthernetFrameType::Debug("from d1 after learning".to_string()),
    );

    let _msg0 = msg0.clone();
    let _msg1 = msg1.clone();
    tokio::spawn(async move {
        d0.send(_msg0).await;

        let pdu = d0.recv().await;
        assert_eq!(pdu, _msg1);
    });

    let _msg0 = msg0.clone();
    let _msg1 = msg1.clone();
    tokio::spawn(async move {
        let pdu = d1.recv().await;

        assert_eq!(pdu, _msg0);

        d1.send(_msg1).await;
    });

    let _msg0 = msg0.clone();
    let _msg1 = msg1.clone();
    tokio::spawn(async move {
        let pdu = d2.recv().await;

        assert_eq!(pdu, msg0);

        tokio::time::sleep(Duration::from_millis(100)).await;

        assert_eq!(d2.received().await, false);

        d2.com.call(Command::Shutdown).await;
    });

    core.run().await;
}
