use crate::VoxFSErrorConvertible;
use alloc::vec::Vec;

/// Implementors can define an error struct if they wish but they must implement methods to read and write from a physical disk or image file.
/// Locations and addresses should all be in bytes
pub trait DiskHandler<E: VoxFSErrorConvertible> {
    /// Write a vector of bytes to a location
    fn write_bytes(&mut self, bytes: &Vec<u8>, location: u64) -> Result<(), E>;

    /// Read an amount of bytes from a location. If the resulting vector is not equal to the amount request the calling function will return an error.
    fn read_bytes(&self, location: u64, amount: u64) -> Result<Vec<u8>, E>;

    /// This method should zero a range between two locations. Start should be inclusive whilst end should be exclusive.
    fn zero_range(&mut self, start: u64, end: u64) -> Result<(), E>;

    /// This should return the raw disk size.
    fn disk_size(&self) -> Result<u64, E>;
}
