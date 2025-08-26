use std::{collections::HashMap, iter::repeat_with};

use macaddr::MacAddr6;

use crate::{Com, Core, Pdu};

// L2スイッチのポートはMacアドレスを持つのかと思ったが、もし持っていると経路制御できないのでMacアドレスは持たないとわかる
// L2レイヤーと言ってはいるが、L2のプロトコルで動くだけでL2の情報を必ずしも持つ必要はない訳ね
pub struct L2Sw {
    coms: Vec<Com>,
    rtb: HashMap<MacAddr6, usize>,
}

impl L2Sw {
    pub fn new(core: &mut Core, n_ports: usize) -> Self {
        L2Sw {
            coms: repeat_with(|| core.com()).take(n_ports).collect(),
            rtb: HashMap::new(),
        }
    }

    pub fn run(mut self) {
        tokio::spawn(async move {
            loop {
                for i in 0..self.coms.len() {
                    if self.coms[i].received() {
                        let pdu = self.coms[i].recv().await;

                        match pdu {
                            Pdu::EthernetFrame(ef) => {
                                self.rtb.insert(ef.src, i);

                                if let Some(dst_com_idx) = self.rtb.get(&ef.dst) {
                                    // PDUの宛先MACアドレスに対応するポートを学習済み

                                    // rtbに格納されるvalueはcoms.lenの範囲なのでcomsへのアクセスが範囲外エラーになることは明らかにないはず
                                    let dst_com =
                                        self.coms.get_mut(*dst_com_idx).expect("unreachable!");
                                    dst_com.send(Pdu::EthernetFrame(ef)).await;
                                } else {
                                    // 学習済みでない

                                    // 送信元ポート以外の全ポートからPDUを送出
                                    for j in 0..self.coms.len() {
                                        if j == i {
                                            let com = self.coms.get_mut(j).expect("unreachable!");
                                            com.send(Pdu::EthernetFrame(ef.clone())).await;
                                        }
                                    }
                                }
                            }
                            _ => unreachable!(),
                        }
                    }
                }

                tokio::task::yield_now().await;
            }
        });
    }
}
