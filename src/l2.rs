use crate::core::*;
use crate::format::*;

use std::collections::HashMap;

pub struct L2 {
    ports: Vec<Device>,
    mactbl: HashMap<u32, Mac>,
}

impl L2 {
    pub fn new(world: &mut World, n_ports: u32) -> Self {
        L2 {
            ports: (0..n_ports).into_iter().map(|_| world.port()).collect(),
            mactbl: HashMap::new(),
        }   
    }

    pub fn run(mut self) {
        tokio::spawn(async move {
            loop {
                let recvs: Vec<_> = {
                    let mut tmp = Vec::new();
                    for (n, port) in &mut self.ports.iter_mut().enumerate() {
                        if let Some(recv) = port.recv_nonblocking().await {
                            tmp.push((n, recv));
                        }
                    }
                    tmp
                };

                for (n_port, (n, recv)) in recvs.into_iter().enumerate() {
                    // println!("processing {:?}", recv);
                    self.mactbl.insert(n as u32, recv.src.clone());

                    // Macアドレステーブルに学習済みのポートが存在する場合、そのポートにのみフレームを流す
                    if let Some(dstport) = self.mactbl.iter().find(|(_, mac)| **mac == recv.dst).map(|(n_port, _)| n_port) {
                        // println!("learned");
                        self.ports.get_mut(*dstport as usize).expect("l2 send failed: unreachable!").send(recv).await;
                    } else {
                        // 学習済みでない場合、受信ポートを除くすべてのポートにフレームを流す
                        for (i, port) in self.ports.iter_mut().enumerate() {
                            if i != n {
                                port.send(recv.clone()).await;
                            }
                        }
                    }
                }

                // スレッドロックしないよう、tokioランタイムに制御を戻す
                tokio::task::yield_now().await;
            }
        });
    }

    pub fn n_port(&mut self, n_port: usize) -> Option<&Device> {
        self.ports.get(n_port)
    }
}