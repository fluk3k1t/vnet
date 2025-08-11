use crate::core::*;
use crate::format::*;

use std::collections::HashMap;
use std::net::Ipv4Addr;

pub struct Router {
    ip: Ipv4Addr,
    ports: HashMap<u32, Device>,
}

impl Router {
    pub fn new(world: &mut World, ip: impl Into<Ipv4Addr>, n_ports: u32) -> Self {
        Router {
            ip: ip.into(),
            ports: (0..n_ports).into_iter().map(|_| world.port()).collect(),
        }
    }
}