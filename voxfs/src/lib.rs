#![no_std]

extern crate alloc;

mod aidfs_error;
mod bitmap;
mod byte_serializable;
mod checksum_trait;
mod disk;
mod manager;
mod utils;

pub use aidfs_error::{AidFSError, AidFSErrorConvertible};
pub use byte_serializable::ByteSerializable;
pub use checksum_trait::Checksum;
pub use disk::*;
pub use manager::OSManager;
