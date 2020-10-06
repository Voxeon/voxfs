use crate::{Disk, VoxFSErrorConvertible};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct DiskInfo {
    number_of_tags: u64,
    free_tag_slots: u64,
    number_of_files: u64,
    free_file_slots: u64,
    block_size: u64,
    free_block_count: u64,
    free_block_space: u64,
}

impl DiskInfo {
    pub fn from_disk<E: VoxFSErrorConvertible>(disk: &Disk<E>) -> Self {
        return Self {
            number_of_tags: disk.number_of_tags() as u64,
            free_tag_slots: disk.free_tag_slots() as u64,
            number_of_files: disk.number_of_files() as u64,
            free_file_slots: disk.free_file_slots() as u64,
            block_size: disk.block_size(),
            free_block_count: disk.free_block_count() as u64,
            free_block_space: disk.free_block_space(),
        };
    }

    #[inline]
    pub fn number_of_tags(&self) -> u64 {
        return self.number_of_tags;
    }

    #[inline]
    pub fn free_tag_slots(&self) -> u64 {
        return self.free_tag_slots;
    }

    #[inline]
    pub fn number_of_files(&self) -> u64 {
        return self.number_of_files;
    }

    #[inline]
    pub fn free_file_slots(&self) -> u64 {
        return self.free_file_slots;
    }

    #[inline]
    pub fn block_size(&self) -> u64 {
        return self.block_size;
    }

    #[inline]
    pub fn free_block_count(&self) -> u64 {
        return self.free_block_count;
    }

    #[inline]
    pub fn free_block_space(&self) -> u64 {
        return self.free_block_space;
    }
}
