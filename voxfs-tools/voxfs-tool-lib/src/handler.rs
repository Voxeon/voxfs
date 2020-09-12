use crate::error::MKImageError;
use std::cell::RefCell;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use voxfs::DiskHandler;

pub struct Handler {
    file: RefCell<File>,
}

impl Handler {
    /// This will create a new file of the specified size. It will overwrite any existing file
    pub fn new_create(path: String, size: usize) -> Result<Self, MKImageError> {
        // Create the file.
        let mut file = match OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
        {
            Ok(f) => f,
            Err(e) => {
                return Err(MKImageError::new(&format!(
                    "Failed to create. Error: {}",
                    e
                )))
            }
        };

        // Make the file up to the size
        let mut remaining = size;

        while remaining > 100 * 1024 * 1024 {
            match file.write_all(&vec![0u8; 100 * 1024 * 1024]) {
                Ok(_) => (),
                Err(e) => {
                    return Err(MKImageError::new(&format!(
                        "Failed to write null bytes. Error: {}",
                        e
                    )))
                }
            }

            remaining -= 100 * 1024 * 1024;
        }

        match file.write_all(&vec![0u8; remaining]) {
            Ok(_) => (),
            Err(e) => {
                return Err(MKImageError::new(&format!(
                    "Failed to write null bytes. Error: {}",
                    e
                )))
            }
        }

        return Ok(Self {
            file: RefCell::new(file),
        });
    }

    // Opens a file
    pub fn new(path: String) -> Result<Self, MKImageError> {
        let file = match OpenOptions::new()
            .read(true)
            .write(true)
            .create(false)
            .open(path.clone())
        {
            Ok(f) => f,
            Err(e) => {
                return Err(MKImageError::new(&format!(
                    "Failed to open file {}. Error: {}",
                    path, e
                )))
            }
        };

        return Ok(Self {
            file: RefCell::new(file),
        });
    }
}

impl DiskHandler<MKImageError> for Handler {
    fn write_bytes(&mut self, bytes: &Vec<u8>, location: u64) -> Result<(), MKImageError> {
        if self.disk_size()? < location + bytes.len() as u64 {
            return Err(MKImageError::new(&format!(
                "File is not large enough to write to address: {}",
                location
            )));
        }

        let mut file = self.file.borrow_mut();

        match file.seek(SeekFrom::Start(location)) {
            Ok(_) => (),
            Err(e) => {
                return Err(MKImageError::new(&format!(
                    "Failed to seek to location: {}. Error: {}",
                    location, e
                )))
            }
        }

        match file.write_all(bytes) {
            Ok(_) => (),
            Err(e) => {
                return Err(MKImageError::new(&format!(
                    "Failed to write to location: {}. Error: {}",
                    location, e
                )))
            }
        }

        return Ok(());
    }

    fn read_bytes(&self, location: u64, amount: u64) -> Result<Vec<u8>, MKImageError> {
        if self.disk_size()? < location + amount {
            return Err(MKImageError::new(&format!(
                "File is not large enough to read address: {}",
                location + amount
            )));
        }

        let mut file = self.file.borrow_mut();

        match file.seek(SeekFrom::Start(location)) {
            Ok(_) => (),
            Err(e) => {
                return Err(MKImageError::new(&format!(
                    "Failed to seek to location: {}. Error: {}",
                    location, e
                )))
            }
        }

        let mut result = vec![0u8; amount as usize];
        match file.read_exact(&mut result) {
            Ok(_) => (),
            Err(e) => {
                return Err(MKImageError::new(&format!(
                    "Failed to read bytes. Error: {}",
                    e
                )))
            }
        }

        return Ok(result);
    }

    fn zero_range(&mut self, start: u64, end: u64) -> Result<(), MKImageError> {
        return self.write_bytes(&vec![0u8; (end - start) as usize], start);
    }

    fn disk_size(&self) -> Result<u64, MKImageError> {
        let b = self.file.borrow();
        let metadata = match b.metadata() {
            Ok(m) => m,
            Err(e) => {
                return Err(MKImageError::new(&format!(
                    "Could not determine file size. Error: {}",
                    e
                )))
            }
        };

        return Ok(metadata.len());
    }
}
