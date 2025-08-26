use std::{collections::HashMap, iter::repeat_with};

use macaddr::MacAddr6;

use crate::{Com, Core, EthernetFrame, EthernetFrameType, HasUuid, Pdu, Uuid};

// L2スイッチのポートはMacアドレスを持つのかと思ったが、もし持っていると経路制御できないのでMacアドレスは持たないとわかる
// L2レイヤーと言ってはいるが、L2のプロトコルで動くだけでL2の情報を必ずしも持つ必要はない訳ね
pub struct L2Sw {
    coms: Vec<Com>,
    rtb: HashMap<MacAddr6, usize>,
}

pub struct L2SwHasUuid {
    uuid: Uuid,
}

impl L2Sw {
    pub fn new(core: &mut Core, n_ports: usize) -> Self {
        L2Sw {
            coms: repeat_with(|| core.com()).take(n_ports).collect(),
            rtb: HashMap::new(),
        }
    }

    pub fn com(&self, n_port: usize) -> L2SwHasUuid {
        L2SwHasUuid {
            uuid: self
                .coms
                .get(n_port)
                .expect("you tried to get uuid from unexisted com port!")
                .uuid,
        }
    }

    pub fn run(mut self) {
        tokio::spawn(async move {
            loop {
                for i in 0..self.coms.len() {
                    if self.coms[i].received() {
                        let ef = self.coms[i].recv().await;

                        self.rtb.insert(ef.src, i);

                        println!("l2 received {:?}", ef);

                        if let Some(dst_com_idx) = self.rtb.get(&ef.dst) {
                            // PDUの宛先MACアドレスに対応するポートを学習済み

                            // rtbに格納されるvalueはcoms.lenの範囲なのでcomsへのアクセスが範囲外エラーになることは明らかにないはず
                            let dst_com = self.coms.get_mut(*dst_com_idx).expect("unreachable!");
                            dst_com.send(ef).await;
                        } else {
                            // 学習済みでない

                            // 送信元ポート以外の全ポートからPDUを送出
                            for j in 0..self.coms.len() {
                                if j != i {
                                    let com = self.coms.get_mut(j).expect("unreachable!");
                                    com.send(ef.clone()).await;
                                }
                            }
                        }
                    }
                }

                tokio::task::yield_now().await;
            }
        });
    }
}

impl HasUuid for L2SwHasUuid {
    fn uuid(&self) -> Uuid {
        self.uuid
    }
}

pub struct EthernetCard {
    mac: MacAddr6,
    com: Com,
    is_promiscuous: bool,
}

impl EthernetCard {
    pub fn new(core: &mut Core, mac: MacAddr6, is_promiscuous: bool) -> Self {
        EthernetCard {
            mac,
            com: core.com(),
            is_promiscuous,
        }
    }

    pub async fn send(&mut self, dst: MacAddr6, payload: EthernetFrameType) {
        let ef = EthernetFrame::new(self.mac, dst, payload);

        self.com.send(ef).await;
    }

    pub async fn recv(&mut self) -> Option<EthernetFrame> {
        let ef = self.com.recv().await;

        if self.is_promiscuous || ef.dst == self.mac || ef.dst.is_broadcast() {
            return Some(ef);
        } else {
            return None;
        }
    }
}
