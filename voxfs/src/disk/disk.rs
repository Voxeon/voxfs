use super::disk_blocks::SuperBlock;
use super::DiskHandler;
use crate::bitmap::BitMap;
use crate::disk::disk_blocks::{INode, INodeFlags, TagBlock, TagFlags};
use crate::{ByteSerializable, OSManager, VoxFSError, VoxFSErrorConvertible};
use alloc::{vec, vec::Vec};

const DEFAULT_BLOCK_SIZE: u64 = 4_096; // In bytes. 4KiB.

// Disk padding size 128 bytes

pub struct Disk<'a, 'b, E: VoxFSErrorConvertible> {
    handler: &'a mut dyn DiskHandler<E>,
    manager: &'b mut dyn OSManager,

    super_block: SuperBlock,

    tag_bitmap: BitMap,
    inode_bitmap: BitMap,
    block_bitmap: BitMap,

    block_size: u64,

    blocks_for_tag_map: u64,
    blocks_for_inode_map: u64,
    blocks_for_block_map: u64,

    tags: Vec<TagBlock>,
}

macro_rules! unwrap_error_aidfs_convertible {
    ($v:expr) => {
        match $v {
            Ok(val) => val,
            Err(e) => return Err(e.into_voxfs_error()),
        }
    };
}

impl<'a, 'b, E: VoxFSErrorConvertible> Disk<'a, 'b, E> {
    pub fn make_new_filesystem(
        handler: &'a mut dyn DiskHandler<E>,
        manager: &'b mut dyn OSManager,
    ) -> Result<Self, VoxFSError<E>> {
        let default_root_tag = TagBlock::new(
            0,
            "root",
            TagFlags::new(true, true),
            manager.current_time().timestamp_nanos() as u64,
            0x0,
            0x0,
            [0u64; 12],
        );

        return Self::make_new_filesystem_with_root(handler, manager, default_root_tag);
    }

    pub fn make_new_filesystem_with_root(
        handler: &'a mut dyn DiskHandler<E>,
        manager: &'b mut dyn OSManager,
        root_tag: TagBlock,
    ) -> Result<Self, VoxFSError<E>> {
        let disk_size = unwrap_error_aidfs_convertible!(handler.disk_size());
        let block_size = DEFAULT_BLOCK_SIZE;

        if block_size % 64 != 0 {
            return Err(VoxFSError::InvalidBlockSize);
        }

        let mut super_block = SuperBlock::new(block_size, disk_size);
        unwrap_error_aidfs_convertible!(handler.zero_range(0, DEFAULT_BLOCK_SIZE)); // Zero the first block.

        let mut offset = DEFAULT_BLOCK_SIZE;

        // Create the bit maps
        let mut tag_bitmap = BitMap::new(super_block.tag_count() as usize);
        let inode_bitmap = BitMap::new(super_block.inode_count() as usize);
        let block_bitmap = BitMap::new(super_block.block_count() as usize);

        macro_rules! bitmap_rounded_to_alignment {
            ($bitmap:expr, $block_size:expr) => {{
                if ($bitmap.len() as u64 % ($block_size * 8)) != 0 {
                    $bitmap.len() as u64 / ($block_size * 8) + 1
                } else {
                    $bitmap.len() as u64 / ($block_size * 8)
                }
            }};
        }

        let blocks_for_tag_map = bitmap_rounded_to_alignment!(tag_bitmap, DEFAULT_BLOCK_SIZE);
        let blocks_for_inode_map = bitmap_rounded_to_alignment!(inode_bitmap, DEFAULT_BLOCK_SIZE);
        let blocks_for_block_map = bitmap_rounded_to_alignment!(block_bitmap, DEFAULT_BLOCK_SIZE);

        offset +=
            (blocks_for_tag_map + blocks_for_inode_map + blocks_for_block_map) * DEFAULT_BLOCK_SIZE;

        // Setup the addresses
        super_block.tag_start_address = offset;
        offset += DEFAULT_BLOCK_SIZE * super_block.blocks_for_tags();

        super_block.inode_start_address = offset;
        offset += DEFAULT_BLOCK_SIZE * super_block.blocks_for_inodes();

        super_block.data_start_address = offset;

        // Write the super block
        unwrap_error_aidfs_convertible!(handler.write_bytes(&super_block.to_bytes().to_vec(), 0));

        let mut new_disk = Self {
            handler,
            manager,
            super_block,
            tag_bitmap,
            inode_bitmap,
            block_bitmap,
            block_size,
            blocks_for_tag_map,
            blocks_for_inode_map,
            blocks_for_block_map,
            tags: vec![root_tag],
        };

        // Write the root tag
        new_disk.store_tag_first_free(root_tag)?;

        // Write the bit maps
        new_disk.write_bitmaps()?;

        return Ok(new_disk);
    }

    pub fn write_bitmaps(&mut self) -> Result<(), VoxFSError<E>> {
        // Write the tags bitmap
        unwrap_error_aidfs_convertible!(self
            .handler
            .write_bytes(&self.tag_bitmap.as_bytes(), self.block_size)); // We start at blocksize because of the superblock

        // Write the inodes bitmap
        unwrap_error_aidfs_convertible!(self.handler.write_bytes(
            &self.inode_bitmap.as_bytes(),
            self.block_size + self.blocks_for_tag_map
        )); // We start at blocksize because of the superblock then skip the tag map

        // Write the data bitmap
        unwrap_error_aidfs_convertible!(self.handler.write_bytes(
            &self.block_bitmap.as_bytes(),
            self.block_size + self.blocks_for_tag_map + self.blocks_for_inode_map
        )); // We start at blocksize because of the superblock then skip the tag map and inode map

        return Ok(());
    }

    pub fn store_tag_first_free(&mut self, tag: TagBlock) -> Result<(), VoxFSError<E>> {
        let index = match self.tag_bitmap.find_next_0_index() {
            Some(index) => index,
            None => return Err(VoxFSError::NoFreeInode),
        };

        unwrap_error_aidfs_convertible!(self.handler.write_bytes(
            &tag.to_bytes().to_vec(),
            self.super_block.tag_start_address + (index as u64) * TagBlock::size()
        ));

        if !self.tag_bitmap.set_bit(index, true) {
            panic!("Unexpected fail."); // This should never happen but if it does then its a developer error so panic.
        }

        self.write_bitmaps()?;

        return Ok(());
    }

    pub fn list_tags(&self) -> Vec<TagBlock> {
        return self.tags.clone();
    }

    pub fn list_inodes(&self) -> Result<Vec<INode>, VoxFSError<E>> {
        let mut nodes = Vec::new();

        for i in 0..self.tag_bitmap.len() {
            if self.tag_bitmap.bit_at(i).unwrap() {
                let location = self.super_block.inode_start_address + (i as u64) * INode::size();

                nodes.push(
                    match INode::from_bytes(&unwrap_error_aidfs_convertible!(self
                        .handler
                        .read_bytes(location, INode::size())))
                    {
                        Some(node) => node,
                        None => return Err(VoxFSError::CorruptedINode),
                    },
                );
            }
        }

        return Ok(nodes);
    }

    fn load_tags(&self) -> Result<Vec<TagBlock>, VoxFSError<E>> {
        let mut tags = Vec::new();

        for i in 0..self.tag_bitmap.len() {
            if self.tag_bitmap.bit_at(i).unwrap() {
                let location = self.super_block.tag_start_address + (i as u64) * TagBlock::size();

                tags.push(
                    match TagBlock::from_bytes(&unwrap_error_aidfs_convertible!(self
                        .handler
                        .read_bytes(location, TagBlock::size())))
                    {
                        Some(tag) => tag,
                        None => return Err(VoxFSError::CorruptedTag),
                    },
                );
            }
        }

        return Ok(tags);
    }

    pub fn create_new_file_first_free(
        &mut self,
        file_inode: INode,
        contents: Vec<u8>,
    ) -> Result<(), VoxFSError<E>> {
        let inode_index = match self.inode_bitmap.find_next_0_index() {
            Some(index) => index,
            None => return Err(VoxFSError::NoFreeInode),
        };

        let blocks = match self.find_blocks(contents.len() as u64) {
            Some(extents) => extents,
            None => return Err(VoxFSError::NotEnoughFreeDataBlocks),
        };

        let mut contents_offset = 0;

        for (start, end) in blocks {
            for i in start..=end {
                if self.block_bitmap.bit_at(i as usize).unwrap() {
                    return Err(VoxFSError::BlockAlreadyAllocated);
                } else {
                    self.block_bitmap.set_bit(i as usize, true);

                    if contents_offset + self.block_size > contents.len() as u64 {
                        unwrap_error_aidfs_convertible!(self.handler.write_bytes(
                            &contents[contents_offset as usize..].to_vec(),
                            contents_offset
                        ));

                    } else {
                        unwrap_error_aidfs_convertible!(self.handler.write_bytes(
                            &contents[contents_offset as usize
                                ..(contents_offset + self.block_size) as usize]
                                .to_vec(),
                            contents_offset
                        ));
                        
                    }

                    contents_offset += self.block_size;
                }
            }
        }

        // unwrap_error_aidfs_convertible!(self.handler.write_bytes(
        //     &tag.to_bytes().to_vec(),
        //     self.super_block.tag_start_address + (index as u64) * TagBlock::size()
        // ));
        //
        // if !self.tag_bitmap.set_bit(index, true) {
        //     panic!("Unexpected fail."); // This should never happen but if it does then its a developer error so panic.
        // }
        //
        // self.write_bitmaps()?;

        return Ok(());
    }

    /// Locates extents and returns a vector of tuples where .0 is the start address and .1 is the end address.
    /// min_size: The minimum size needed in BYTES
    fn find_blocks(&self, min_size: u64) -> Option<Vec<(u64, u64)>> {
        let num_blocks_required = {
            if min_size % self.block_size != 0 {
                1 + min_size / self.block_size
            } else {
                min_size / self.block_size
            }
        };

        // Check if we have enough blocks.
        if (self
            .block_bitmap
            .count_zeros_up_to(self.super_block.block_count() as usize))
        .unwrap()
            < num_blocks_required as usize
        {
            return None;
        }

        let mut res = Vec::new();
        let mut blocks_found = 0;

        let mut first_index = self.block_bitmap.find_next_0_index().unwrap() as u64;

        // Find the largest available extent first them work down to individual blocks.
        while blocks_found < num_blocks_required {
            // Find largest available extent up to the size required

            let mut start_index = first_index;
            let mut end_index = start_index;

            let mut largest_start_index = start_index;
            let mut largest_end_index = end_index;

            let mut i = start_index + 1;

            // Find the largest available extent up to the required size
            while i < self.super_block.block_count() {
                if (end_index - start_index + 1) >= (num_blocks_required - blocks_found) {
                    // We add one here to account for the inclusive end of the extent
                    largest_start_index = start_index;
                    largest_end_index = end_index;
                    break;
                }

                if !self.block_bitmap.bit_at(i as usize).unwrap() {
                    end_index += 1;
                } else {
                    if (end_index - start_index) > (largest_end_index - largest_start_index) {
                        largest_start_index = start_index;
                        largest_end_index = end_index;
                    }

                    start_index = i + 1;
                    end_index = i + 1;
                }

                i += 1;
            }

            blocks_found += largest_end_index - largest_start_index + 1;
            res.push((largest_start_index, largest_end_index));

            if largest_start_index == first_index {
                let mut new_first_index = None;

                for i in largest_end_index..self.super_block.block_count() {
                    if !self.block_bitmap.bit_at(i as usize).unwrap() {
                        new_first_index = Some(i);
                    }
                }

                match new_first_index {
                    Some(i) => first_index = i,
                    None => return None, // Should never happen but just in case.
                }
            }
        }

        return Some(res);
    }
}
