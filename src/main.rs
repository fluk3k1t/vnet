use std::{thread::sleep, time::Duration};

use vnet::{Command, Core, Device, Nic, Pdu};

#[tokio::main]
pub async fn main() {
    let mut core = Core::new();

    tokio::spawn(async move {});

    tokio::spawn(async move {});

    core.run().await;
}
