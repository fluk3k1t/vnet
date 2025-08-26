use macaddr::MacAddr6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EthernetFrame<T> {
    pub dst: MacAddr6,
    pub src: MacAddr6,
    pub payload: T,
}

