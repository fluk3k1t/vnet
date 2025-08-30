use std::{
    collections::{HashMap, VecDeque},
    iter::repeat_with,
    net::Ipv4Addr,
    ops::Index,
    sync::Arc,
};

use actix::prelude::*;
use futures::future::join_all;
use macaddr::MacAddr6;
use tokio::sync::Mutex;
use tracing::debug;

use crate::{
    Connectable, Core, EndPoint, EthernetCard, EthernetFrame, EthernetFrameType, IPv4PacketType,
    OnReceive, Uuid,
};

