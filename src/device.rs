use crate::{Com, Core, HasCom, Pdu, Uuid};

pub struct Device {
    pub com: Com,
}

impl Device {
    pub fn new(core: &mut Core) -> Self {
        Device { com: core.com() }
    }

    pub async fn send(&mut self, payload: Pdu) {
        self.com.send(payload).await
    }

    pub async fn recv(&mut self) -> Pdu {
        self.com.recv().await
    }
}

impl HasCom for Device {
    fn com(&self) -> &Com {
        &self.com
    }
}
