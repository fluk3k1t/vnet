use std::{thread::sleep, time::Duration};

use tokio::{io::join, join};
use vnet::{Actor, Core, CreateCom, Handler, Send};

#[tokio::main]
async fn main() {
    let (mut core, core_join) = Core::new().start();

    tokio::spawn(async move {
        let mut com = core.call(CreateCom).await;
        // println!("{:?}", com);

        let r = com
            .call(Send {
                payload: "hello from main loop".to_string(),
            })
            .await
            .await;
        println!("{}", r);
    });

    sleep(Duration::from_secs(1));
}
