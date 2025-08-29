use actix::prelude::*;
use tracing::{Level, debug};
use vnet::Core;

#[actix::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_file(true)
        .with_line_number(true)
        .with_thread_names(true)
        .with_level(true)
        .with_max_level(Level::DEBUG)
        .init();

    let mut core = Core::new();

    // let mut com = core.create_ep().await;

    // com.send().await;
}
