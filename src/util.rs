use const_random::const_random;
use macaddr::MacAddr6;
use rand::Rng;

pub fn mac_rnd() -> MacAddr6 {
    let mut rng = rand::rng();
    let mut b = [0u8; 6];
    rng.fill(&mut b);

    b[0] = (b[0] & 0b1111_1100) | 0b0000_0010;

    if b == [0; 6] || b == [0xFF; 6] {
        b[5] ^= 0x01;
    }

    MacAddr6::from(b)
}
