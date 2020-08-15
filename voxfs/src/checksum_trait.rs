use super::ByteSerializable;

pub trait Checksum {
    fn set_checksum(&mut self);

    fn calculate_checksum(&self) -> u8
    where
        Self: ByteSerializable,
    {
        let raw_bytes = self.to_bytes();
        let bytes = Self::generic_bytes_rep(&raw_bytes);

        let mut sum = 0u8;

        for b in bytes.iter() {
            sum = sum.wrapping_add(*b);
        }

        return 0u8.wrapping_sub(sum);
    }

    fn perform_checksum(&self) -> bool
    where
        Self: ByteSerializable,
    {
        let raw_bytes = self.to_bytes();
        let bytes = Self::generic_bytes_rep(&raw_bytes);

        let mut sum = 0u8;

        for b in bytes.iter() {
            sum = sum.wrapping_add(*b);
        }

        return sum == 0;
    }
}
