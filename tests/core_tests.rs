use vnet::{Command, Core, Device, Pdu};

#[tokio::test]
async fn test_com() {
    let mut core = Core::new();

    let mut d0 = Device::new(&mut core);
    let mut d1 = Device::new(&mut core);

    core.connect(&d0, &d1);

    tokio::spawn(async move {
        let r = d1.recv().await;
        assert_eq!(r, Pdu::Dummy);
        d1.com.call(Command::Shutdown).await;
    });

    tokio::spawn(async move {
        d0.send(Pdu::Dummy).await;
    });

    core.run().await;
}

#[tokio::test]
async fn test_com_unconnected() {
    let mut core = Core::new();

    let mut d0 = Device::new(&mut core);
    let mut d1 = Device::new(&mut core);

    tokio::spawn(async move {
        d0.send(Pdu::Dummy).await;
    });

    tokio::spawn(async move {
        let res = tokio::time::timeout(std::time::Duration::from_millis(100), d1.recv()).await;

        assert!(res.is_err(), "unexpectedly received: {:?}", res);

        d1.com.call(Command::Shutdown).await;
    });

    core.run().await;
}
