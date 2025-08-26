use crate::{Com, Core, HasUuid, Pdu, Uuid};

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

    pub async fn received(&mut self) -> bool {
        self.com.received()
    }
}

impl HasUuid for Device {
    fn uuid(&self) -> Uuid {
        self.com.uuid
    }
}
