use std::time::Duration;

use actix::prelude::*;
use macaddr::MacAddr6;
use tokio::{join, time::sleep};
use tracing::{Level, debug};
use vnet::{Core, EthernetCard, EthernetFrameType, L2Sw};

#[actix::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_file(true)
        .with_line_number(true)
        .with_thread_names(true)
        .with_level(true)
        .with_max_level(Level::DEBUG)
        .init();
}
