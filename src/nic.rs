use crate::Com;

pub struct Nic {
    com: Com,
}

impl Nic {
    pub fn new(com: Com) -> Self {
        Nic { com }
    }

    pub fn run(mut self) {
        tokio::spawn(async move {
            loop {
                let pdu = self.com.recv().await;
                // match pdu {

                // }
            }
        });
    }
}

pub struct NicHandler {}
