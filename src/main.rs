use tokio::{io::join, join};
use vnet::{Actor, Core, CreateCom, Handler};

#[tokio::main]
async fn main() {
    let (mut core, core_join) = Core::new().start();

    let mut res = core.call(CreateCom).await;

    println!("{}", res);

    core_join.await.unwrap();
}
