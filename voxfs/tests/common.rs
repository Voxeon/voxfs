extern crate voxfs;
use chrono::{DateTime, Utc};
use voxfs::{DiskHandler, OSManager, VoxFSErrorConvertible};

#[derive(Debug)]
pub struct Error {}

impl VoxFSErrorConvertible for Error {}

pub struct Handler {
    pub disk: Vec<u8>,
}

impl Handler {
    pub fn new(disk_size: usize) -> Self {
        return Self {
            disk: vec![0u8; disk_size],
        };
    }

    pub fn dump_disk(&self) -> Vec<u8> {
        return self.disk.clone();
    }
}

impl DiskHandler<Error> for Handler {
    fn write_bytes(&mut self, bytes: &Vec<u8>, location: u64) -> Result<(), Error> {
        for (i, byte) in bytes.iter().enumerate() {
            self.disk[location as usize + i] = *byte;
        }

        return Ok(());
    }

    fn read_bytes(&self, location: u64, amount: u64) -> Result<Vec<u8>, Error> {
        let location = location as usize;
        let amount = amount as usize;
        return Ok(self.disk[location..location + amount].to_vec());
    }

    fn zero_range(&mut self, start: u64, end: u64) -> Result<(), Error> {
        for i in start..end {
            self.disk[i as usize] = 0;
        }

        return Ok(());
    }

    fn disk_size(&self) -> Result<u64, Error> {
        return Ok(self.disk.len() as u64);
    }
}

#[derive(Debug)]
pub struct Manager {}

impl Manager {
    pub fn new() -> Self {
        return Self {};
    }
}

impl OSManager for Manager {
    fn current_time(&self) -> DateTime<Utc> {
        return Utc::now();
    }
}
