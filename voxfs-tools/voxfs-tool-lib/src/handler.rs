use voxfs::DiskHandler;
use crate::error::MKImageError;
use std::fs::File;
use std::io::{Write, SeekFrom, Read, Seek};
use std::cell::RefCell;

pub struct Handler {
    file: RefCell<File>,
}

impl Handler {
    /// This will create a new file of the specified size
    pub fn new_create(path: String, size: usize) -> Result<Self, MKImageError> {
        // Create the file.
        let mut file = match File::create(path) {
            Ok(f) => f,
            Err(e) => return Err(MKImageError::new(&format!("Failed to create. Error: {}", e))),
        };

        // Make the file up to the size
        match file.write_all(&vec![0u8; size]) {
            Ok(_) => (),
            Err(e) => return Err(MKImageError::new(&format!("Failed to write null bytes. Error: {}", e))),
        }

        return Ok(Self {file: RefCell::new(file)});
    }

    // Opens a file
    pub fn new(path: String) -> Result<Self, MKImageError> {
        let mut file = match File::open(path.clone()) {
            Ok(f) => f,
            Err(e) => return Err(MKImageError::new(&format!("Failed to open file {}. Error: {}", path, e))),
        };

        return Ok(Self {file: RefCell::new(file)});
    }
}

impl DiskHandler<MKImageError> for Handler {
    fn write_bytes(&mut self, bytes: &Vec<u8>, location: u64) -> Result<(), MKImageError> {
        unimplemented!()
    }

    fn read_bytes(&self, location: u64, amount: u64) -> Result<Vec<u8>, MKImageError> {
        if self.disk_size()? < location + amount {
            return Err(MKImageError::new(&format!("File is not large enough to read address: {}", location + amount)));
        }

        let mut file = self.file.borrow_mut();

        match file.seek(SeekFrom::Start(location)) {
            Ok(_) => (),
            Err(e) => return Err(MKImageError::new(&format!("Failed to seek to location: {}. Error: {}", location, e))),
        }

        let mut result = vec![0u8; amount as usize];
        match file.read_exact(&mut result) {
            Ok(_) => (),
            Err(e) => return Err(MKImageError::new(&format!("Failed to read bytes. Error: {}", e))),
        }

        return Ok(result);
    }

    fn zero_range(&mut self, start: u64, end: u64) -> Result<(), MKImageError> {
        unimplemented!()
    }

    fn disk_size(&self) -> Result<u64, MKImageError> {
        let b = self.file.borrow();
        let metadata = match b.metadata() {
            Ok(m) => m,
            Err(e) => return Err(MKImageError::new(&format!("Could not determine file size. Error: {}", e)))
        };

        return Ok(metadata.len());
    }
}