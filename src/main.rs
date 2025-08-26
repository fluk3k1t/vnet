use std::{thread::sleep, time::Duration};

use vnet::{Command, Core, Device, Stream};

#[tokio::main]
pub async fn main() {
    let mut core = Core::new();

    let mut d0 = Device::new(&mut core);
    let mut d1 = Device::new(&mut core);

    core.connect(&d0, &d1);

    let core_task = tokio::spawn(core.run());

    let d0_task = tokio::spawn(async move {
        d0.send(Stream::Ipv4).await;
    });

    let d1_task = tokio::spawn(async move {
        let r = d1.recv().await;
        println!("{:?}", r);
        d1.com.call(Command::Shutdown).await;
    });

    let _ = tokio::join!(core_task, d0_task, d1_task);
}
