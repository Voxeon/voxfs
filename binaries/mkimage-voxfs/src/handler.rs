use voxfs::DiskHandler;
use crate::error::MKImageError;
use std::fs::File;
use std::io::Write;

struct Handler {
    file: File,
}

impl Handler {
    /// This will create a new file of the specified size
    pub fn new(path: String, size: usize) -> Result<Self, MKImageError> {
        let mut file = match File::create(path) {
            Ok(f) => f,
            Err(e) => return Err(MKImageError::new(&format!("Failed to create. Error: {}", e))),
        };

        match file.write_all(&vec![0u8; size]) {
            Ok(_) => (),
            Err(e) => return Err(MKImageError::new(&format!("Failed to write null bytes. Error: {}", e))),
        }

        return Ok(Self {file});
    }
}

impl DiskHandler<MKImageError> for Handler {
    fn write_bytes(&mut self, bytes: &Vec<u8>, location: u64) -> Result<(), MKImageError> {
        unimplemented!()
    }

    fn read_bytes(&self, location: u64, amount: u64) -> Result<Vec<u8>, MKImageError> {
        unimplemented!()
    }

    fn zero_range(&mut self, start: u64, end: u64) -> Result<(), MKImageError> {
        unimplemented!()
    }

    fn disk_size(&self) -> Result<u64, MKImageError> {
        let metadata = match self.file.metadata() {
            Ok(m) => m,
            Err(e) => return Err(MKImageError::new(&format!("Could not determine file size. Error: {}", e)))
        };

        return Ok(metadata.len());
    }
}