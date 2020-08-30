// Disk layout:
// 1024-bit padding block, super-block, inodes (10% of the disk is reserved for inodes),
// tag table (10% of the disk is reserved for tags), data blocks ...

mod disk;
mod disk_blocks;
pub mod disk_handler;

pub use disk::{Disk, FileSize};
pub use disk_blocks::{INode, INodeFlags, IndirectINode, IndirectTagBlock, TagBlock, TagFlags};
pub use disk_handler::DiskHandler;
