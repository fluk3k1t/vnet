pub type Packet = Frame;

#[derive(Clone, Debug, PartialEq)]
pub struct Mac {
    inner: String
}

impl Mac {
    pub fn new(dummy: impl Into<String>) -> Self {
        Mac {
            inner: dummy.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub src: Mac,
    pub dst: Mac,
    pub data: String,
}

impl Frame {
    pub fn new(src: Mac, dst: Mac, data: impl Into<String>) -> Self {
        Frame {
            src,
            dst,
            data: data.into(),
        }
    }
}
