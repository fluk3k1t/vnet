pub mod core;
pub use core::*;

pub mod device;
pub use device::*;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_com() {
        let mut core = Core::new();

        let mut d0 = Device::new(&mut core);
        let mut d1 = Device::new(&mut core);

        core.connect(&d0, &d1);

        tokio::spawn(async move {
            d0.send(Stream::Dummy).await;
        });

        tokio::spawn(async move {
            let r = d1.recv().await;
            assert_eq!(r, Stream::Dummy);
            d1.com.call(Command::Shutdown).await;
        });

        tokio::spawn(core.run());
    }
}
