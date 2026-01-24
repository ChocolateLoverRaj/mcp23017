use core::array;

pub trait FromBits<const BIT_LEN: usize> {
    fn from_bits_le(bits: [bool; BIT_LEN]) -> Self;
}

impl FromBits<{ u8::BITS as usize }> for u8 {
    fn from_bits_le(bits: [bool; u8::BITS as usize]) -> Self {
        bits.iter()
            .enumerate()
            .fold(0u8, |acc, (i, &b)| acc | ((b as u8) << i))
    }
}

pub trait IntoBits<const BIT_LEN: usize> {
    fn into_bits_le(self) -> [bool; BIT_LEN];
}

impl IntoBits<{ u8::BITS as usize }> for u8 {
    fn into_bits_le(self) -> [bool; u8::BITS as usize] {
        array::from_fn(|i| (self & (1 << i)) != 0)
    }
}
