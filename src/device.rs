use crate::{Com, Core, HasCom, Stream, Uuid};

pub struct Device {
    com: Com,
}

impl Device {
    pub fn new(core: &mut Core) -> Self {
        Device { com: core.com() }
    }

    pub async fn send(&mut self, payload: Stream) {
        self.com.send(payload).await
    }

    pub async fn recv(&mut self) -> Stream {
        self.com.recv().await
    }
}

impl HasCom for Device {
    fn com(&self) -> &Com {
        &self.com
    }
}
