use super::disk_blocks::SuperBlock;
use super::DiskHandler;
use crate::bitmap::BitMap;
use crate::disk::disk_blocks::{
    Extent, INode, INodeFlags, IndirectINode, IndirectTagBlock, TagBlock, TagFlags,
};
use crate::{ByteSerializable, DiskInfo, OSManager, VoxFSError, VoxFSErrorConvertible};
use alloc::{string::String, vec, vec::Vec};

const DEFAULT_BLOCK_SIZE: u64 = 4_096; // In bytes. 4KiB.
pub const FORBIDDEN_CHARACTERS: [char; 21] = [
    '#', '<', '$', '+', '%', '>', '!', '`', '&', '*', '\'', '|', '{', '}', '?', '"', '=', '/', ':',
    '\\', '@',
];

pub struct FileSize {
    pub physical_size: u64,
    pub actual_size: u64,
}

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
    #[allow(dead_code)] // This may be needed later but for now it is kept for consistency reasons
    blocks_for_block_map: u64,

    // No guarantees are made about the order of the inodes, they may not be in index order.
    tags: Vec<TagBlock>,
    inodes: Vec<INode>,
}

/// This macro unwraps an error for voxfs. We use a macro here because it takes advantage of templates.
/// This macro returns, the name which suggests that it will cause a panic will NOT cause a panic.
macro_rules! unwrap_return_error_voxfs_convertible {
    ($v:expr) => {
        match $v {
            Ok(val) => val,
            Err(e) => return Err(e.into_voxfs_error()),
        }
    };
}

/// This macro provides a way to round a number of bits as integers to a specific block size
macro_rules! rounded_to_alignment {
    ($n:expr, $block_size:expr) => {{
        if ($n as u64 % ($block_size * 8)) != 0 {
            $n as u64 / ($block_size * 8) + 1
        } else {
            $n as u64 / ($block_size * 8)
        }
    }};
}

impl<'a, 'b, E: VoxFSErrorConvertible> Disk<'a, 'b, E> {
    /// Constructs a new filesystem.
    pub fn make_new_filesystem(
        handler: &'a mut dyn DiskHandler<E>,
        manager: &'b mut dyn OSManager,
    ) -> Result<Self, VoxFSError<E>> {
        let default_root_tag = TagBlock::new(
            0,
            "root",
            TagFlags::new(true, true),
            manager.current_time(),
            0x0,
            0x0,
            [0u64; 12],
        );

        return Self::make_new_filesystem_with_root(handler, manager, default_root_tag);
    }

    /// Constructs a new filesystem with a specified root tag. This is primarily for testing purposes only.
    pub fn make_new_filesystem_with_root(
        handler: &'a mut dyn DiskHandler<E>,
        manager: &'b mut dyn OSManager,
        root_tag: TagBlock,
    ) -> Result<Self, VoxFSError<E>> {
        let disk_size = unwrap_return_error_voxfs_convertible!(handler.disk_size());
        let block_size = DEFAULT_BLOCK_SIZE;

        if block_size % 64 != 0 {
            return Err(VoxFSError::InvalidBlockSize);
        }

        let mut super_block = SuperBlock::new(block_size, disk_size);
        unwrap_return_error_voxfs_convertible!(handler.zero_range(0, block_size)); // Zero the first block.

        // The address of where we can start the data.
        let mut offset = DEFAULT_BLOCK_SIZE;

        // Create the bit maps
        let tag_bitmap = BitMap::new(super_block.tag_count() as usize);
        let inode_bitmap = BitMap::new(super_block.inode_count() as usize);
        let block_bitmap = BitMap::new(super_block.block_count() as usize);

        macro_rules! bitmap_rounded_to_alignment {
            ($bitmap:expr, $block_size:expr) => {
                rounded_to_alignment!($bitmap.len(), block_size)
            };
        }

        // Calculate the number of blocks required for each bitmap, rounded to a block alignment
        let blocks_for_tag_map = bitmap_rounded_to_alignment!(tag_bitmap, block_size);
        let blocks_for_inode_map = bitmap_rounded_to_alignment!(inode_bitmap, block_size);
        let blocks_for_block_map = bitmap_rounded_to_alignment!(block_bitmap, block_size);

        offset += (blocks_for_tag_map + blocks_for_inode_map + blocks_for_block_map) * block_size;

        // Setup the addresses

        // The super block calculates for us then number of blocks for tags, inodes and data blocks
        super_block.set_tag_start_address(offset);
        offset += block_size * super_block.blocks_for_tags();

        super_block.set_inode_start_address(offset);
        offset += block_size * super_block.blocks_for_inodes();

        super_block.set_data_start_address(offset);

        // Write the super block
        unwrap_return_error_voxfs_convertible!(
            handler.write_bytes(&super_block.to_bytes().to_vec(), 0)
        );

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
            inodes: Vec::new(),
        };

        // Write the root tag
        new_disk.store_tag_first_free(root_tag)?;

        // Write the bit maps
        new_disk.write_bitmaps()?;

        return Ok(new_disk);
    }

    /// Gives access to the disk handler
    pub fn handler(&mut self) -> &mut dyn DiskHandler<E> {
        return self.handler;
    }

    /// Returns the number of available data blocks
    pub fn available_data_blocks(&self) -> u64 {
        return self
            .block_bitmap
            .count_zeros_up_to(self.super_block.block_count() as usize)
            .unwrap_or(0) as u64;
    }

    /// Opens a disk, loading the required details
    pub fn open_disk(
        handler: &'a mut dyn DiskHandler<E>,
        manager: &'b mut dyn OSManager,
    ) -> Result<Self, VoxFSError<E>> {
        // Things to do:
        // 1: Load the super block
        // 2: Load the bitmaps

        // Read the super block
        let block_size = DEFAULT_BLOCK_SIZE;
        let first_block = unwrap_return_error_voxfs_convertible!(handler.read_bytes(0, block_size));
        let super_block = match SuperBlock::from_bytes(&first_block) {
            Some(b) => b,
            None => return Err(VoxFSError::CorruptedSuperBlock),
        };

        // Determine the block size and the number of blocks for the bitmaps
        let block_size = super_block.block_size();
        let blocks_for_tag_map = rounded_to_alignment!(super_block.tag_count(), block_size);
        let blocks_for_inode_map = rounded_to_alignment!(super_block.inode_count(), block_size);
        let blocks_for_block_map = rounded_to_alignment!(super_block.block_count(), block_size);

        // Read the data for the bitmaps
        let tag_bitmaps_bytes = unwrap_return_error_voxfs_convertible!(
            handler.read_bytes(block_size, blocks_for_tag_map * block_size)
        );

        let inode_bitmaps_bytes = unwrap_return_error_voxfs_convertible!(handler.read_bytes(
            block_size + blocks_for_tag_map * block_size,
            blocks_for_inode_map * block_size
        ));

        let data_bitmaps_bytes = unwrap_return_error_voxfs_convertible!(handler.read_bytes(
            block_size + (blocks_for_tag_map + blocks_for_inode_map) * block_size,
            blocks_for_block_map * block_size
        ));

        // Construct the bitmaps
        let tag_bitmap = BitMap::from_bytes(&tag_bitmaps_bytes);
        let inode_bitmap = BitMap::from_bytes(&inode_bitmaps_bytes);
        let block_bitmap = BitMap::from_bytes(&data_bitmaps_bytes);

        let mut s = Self {
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
            tags: Vec::new(),
            inodes: Vec::new(),
        };

        // Load the tags and inodes into memory.
        s.tags = s.load_tags()?;
        s.inodes = s.load_inodes()?;

        return Ok(s);
    }

    /// Creates a new tag in the first available slot.
    pub fn create_new_tag(
        &mut self,
        name: &str,
        flags: TagFlags,
    ) -> Result<TagBlock, VoxFSError<E>> {
        self.validate_name(name, VoxFSError::InvalidTagName)?;

        // store_tag_first_free will set the index
        let tag = TagBlock::new(
            0,
            name,
            flags,
            self.manager.current_time(),
            0,
            0,
            [0u64; 12],
        );

        let tag = self.store_tag_first_free(tag)?;
        self.tags.push(tag); // Keep track of the tag in memory

        return Ok(tag);
    }

    /// Deletes a tag for the tag with the specified index
    pub fn delete_tag(&mut self, index: u64) -> Result<(), VoxFSError<E>> {
        let mut local_index = None; // The index of the tag in the vector in memory
        let mut local_tag = None;

        // Locate the tag in the memory map
        for (i, tag) in self.tags.iter().enumerate() {
            if tag.index() == index {
                local_index = Some(i);
                local_tag = Some(tag.clone());
                break;
            }
        }

        // Ensure we found something
        if local_index.is_none() {
            return Err(VoxFSError::CouldNotFindTag);
        }

        // Unwrap them once so we don't need to every time we use them
        let local_index = local_index.unwrap();
        let local_tag = local_tag.unwrap();

        // We use this to track where each indirect block is located in memory
        let mut data_block_indices: Vec<u64> = Vec::new(); // index of data block
        let mut current_indirect = local_tag.indirect_pointer();

        while current_indirect.is_some() {
            // Read the indirect blocks
            let address = current_indirect.unwrap();
            let index = self.address_to_data_index(address);
            let bytes = self.read_from_address(address, self.block_size)?;

            let block = match IndirectTagBlock::from_bytes(&bytes) {
                Some(b) => b,
                None => return Err(VoxFSError::CorruptedIndirectTag),
            };

            // Track the data block index of all the indirect blocks we find to ensure
            // that we mark them all as free
            current_indirect = block.next();
            data_block_indices.push(index);
        }

        // Mark the indirect block spaces as free
        for index in data_block_indices {
            if !self.block_bitmap.set_bit(index as usize, false) {
                return Err(VoxFSError::FailedToFreeBlock);
            }
        }

        // Mark the tag space as free
        if !self.tag_bitmap.set_bit(local_tag.index() as usize, false) {
            return Err(VoxFSError::FailedToFreeTag);
        }

        // Remove from the memory map
        self.tags.remove(local_index);

        // Write the bitmaps to the disk.
        self.write_bitmaps()?;

        return Ok(());
    }

    /// List the tags stored on the disk, this method doesn't reload them.
    pub fn list_tags(&self) -> Vec<TagBlock> {
        return self.tags.clone();
    }

    /// List the inodes on the disk, this method doesn't reload them.
    pub fn list_inodes(&self) -> Vec<INode> {
        return self.inodes.clone();
    }

    /// The number of tags on this disk
    pub fn number_of_tags(&self) -> usize {
        return self.tags.len();
    }

    /// The number of spaces available for new tags
    pub fn free_tag_slots(&self) -> usize {
        return self
            .tag_bitmap
            .count_zeros_up_to(self.super_block.tag_count() as usize)
            .unwrap_or(0);
    }

    /// The number of files on this disk.
    pub fn number_of_files(&self) -> usize {
        return self.inodes.len();
    }

    /// The number of spaces available for new files
    pub fn free_file_slots(&self) -> usize {
        return self
            .inode_bitmap
            .count_zeros_up_to(self.super_block.inode_count() as usize)
            .unwrap_or(0);
    }

    /// The size of the disk blocks
    pub fn block_size(&self) -> u64 {
        return self.block_size;
    }

    /// The number of free blocks
    pub fn free_block_count(&self) -> usize {
        return self
            .block_bitmap
            .count_zeros_up_to(self.super_block.block_count() as usize)
            .unwrap_or(0);
    }

    /// The amount of free data block space
    pub fn free_block_space(&self) -> u64 {
        return (self.free_block_count() as u64) * self.block_size;
    }

    /// Returns the disk info
    pub fn disk_info(&self) -> DiskInfo {
        return DiskInfo::from_disk(self);
    }

    /// Add an inode to a tag
    pub fn apply_tag(&mut self, tag_index: u64, inode_index: u64) -> Result<(), VoxFSError<E>> {
        let inode = self.inodes[self.locate_inode(inode_index)?];

        // Locate the tag in the memory map from the disk index provided
        let mut tag_self_index = None;

        // Find the local index of this tag
        for (i, tag) in self.tags.iter().enumerate() {
            if tag.index() == tag_index {
                tag_self_index = Some(i);
                break;
            }
        }

        // Unwrap it to ensure we could find it
        let tag_self_index = match tag_self_index {
            Some(i) => i,
            None => return Err(VoxFSError::CouldNotFindTag),
        };

        // This checks if we have enough space in the tag block itself to add a new member
        // if not we create a new indirect tag block
        if self.tags[tag_self_index].number_of_pointers() >= TagBlock::MAXIMUM_LOCAL_MEMBERS {
            /*
            Two things must be checked.
            1. Has this tag been applied before to this INode.
            2. Where is a free spot?
             */

            // Check if we have already applied this tag to the inode
            if self.tags[tag_self_index].contains_member(&inode.index()) {
                return Err(VoxFSError::TagAlreadyAppliedToINode);
            }

            let mut next_address = self.tags[tag_self_index].indirect_pointer();
            let mut previous_indirect = None;
            let mut previous_indirect_location = None;
            let mut free_indirect_tag_address: Option<u64> = None; // This tracks the location of an available spot in an inode

            // NOTE: We don't bail early after finding a spot because we need to still check
            // every indirect inode to ensure we don't apply the tag twice.
            while next_address.is_some() {
                let bytes = self.read_from_address(next_address.unwrap(), self.block_size)?;

                let indirect_tag = match IndirectTagBlock::from_bytes(&bytes) {
                    Some(t) => t,
                    None => return Err(VoxFSError::CorruptedIndirectTag),
                };

                // Check if we contain the member already
                if indirect_tag.contains_member(&inode.index()) {
                    return Err(VoxFSError::TagAlreadyAppliedToINode);
                }

                // Only if we haven't found a spot yet add it to this.
                if free_indirect_tag_address.is_none()
                    && (indirect_tag.number_of_members() as u64)
                        < IndirectTagBlock::max_members_for_blocksize(self.block_size)
                {
                    free_indirect_tag_address = next_address;
                }

                let nxt = indirect_tag.next();
                previous_indirect = Some(indirect_tag);
                previous_indirect_location = Some(next_address.unwrap());
                next_address = nxt;
            }

            // If we didn't find a spot to add this tag... Create a new one!
            if free_indirect_tag_address.is_none() {
                // Create a new indirect tag
                let indirect_tag = IndirectTagBlock::new(
                    self.tags[tag_self_index].index(),
                    vec![inode.index()],
                    0,
                    self.block_size,
                );

                // Find a spot for it
                let index = match self.find_block() {
                    Some(index) => index,
                    None => return Err(VoxFSError::NotEnoughFreeDataBlocks),
                };

                let location = self.data_index_to_address(index);

                // Mark the block as taken
                self.block_bitmap.set_bit(index as usize, true);

                // Write the new indirect block
                self.write_to_address(
                    location,
                    &indirect_tag.to_bytes_padded(self.block_size as usize),
                )?;

                // Write the bitmaps
                self.write_bitmaps()?;

                // We need to point to this new block somehow...
                // Set the previous indirect block to point to this or the root tag
                match previous_indirect {
                    Some(mut i) => {
                        i.set_next(location);

                        self.write_to_address(
                            previous_indirect_location.unwrap(),
                            &i.to_bytes_padded(self.block_size as usize),
                        )?;
                    }
                    None => {
                        self.tags[tag_self_index].set_indirect(location);

                        self.write_to_address(
                            self.tag_index_to_address(self.tags[tag_self_index].index()),
                            &self.tags[tag_self_index].to_bytes().to_vec(),
                        )?;
                    }
                }
            } else {
                // Otherwise place it in the free spot

                // Read the indirect block and check that it was valid
                let mut indirect = match IndirectTagBlock::from_bytes(
                    &self.read_from_address(free_indirect_tag_address.unwrap(), self.block_size)?,
                ) {
                    Some(i) => i,
                    None => return Err(VoxFSError::CorruptedIndirectTag),
                };

                // Set the blocksize so we can use the append_member function
                indirect.set_block_size(self.block_size);

                // Append a member
                if !indirect.append_member(inode.index()) {
                    return Err(VoxFSError::FailedIndirectTagAppend);
                }

                // rewrite the new indirect block
                self.write_to_address(
                    free_indirect_tag_address.unwrap(),
                    &indirect.to_bytes_padded(self.block_size as usize),
                )?;
            }
        } else {
            // There's enough space to add it to the tag block itself

            // Check if this tag has already been applied.
            if self.tags[tag_self_index].contains_member(&inode.index()) {
                return Err(VoxFSError::TagAlreadyAppliedToINode);
            }

            self.tags[tag_self_index].append_member(inode.index());

            self.write_to_address(
                self.tag_index_to_address(self.tags[tag_self_index].index()),
                &self.tags[tag_self_index].to_bytes().to_vec(),
            )?;
        }

        return Ok(());
    }

    /// Remove a tag from an inode, automatically deleting an empty indirect tag block.
    pub fn remove_tag_from_inode(
        &mut self,
        tag_index: u64,
        inode_index: u64,
    ) -> Result<(), VoxFSError<E>> {
        return self.remove_tag_from_inode_optional_prune(tag_index, inode_index, true);
    }

    /// Remove a tag from an inode, if prune is true an empty indirect tag block is deleted.
    pub fn remove_tag_from_inode_optional_prune(
        &mut self,
        tag_index: u64,
        inode_index: u64,
        prune: bool,
    ) -> Result<(), VoxFSError<E>> {
        // Locate the inode
        let inode = self.inodes[self.locate_inode(inode_index)?];

        let mut tag = None;
        let mut tag_local_index = None;

        // Locate where the tag is in the memory map
        for (i, t) in self.tags.iter().enumerate() {
            if t.index() == tag_index {
                tag = Some(*t);
                tag_local_index = Some(i);
                break;
            }
        }

        // Ensure we were able to find it
        if tag.is_none() {
            return Err(VoxFSError::CouldNotFindTag);
        }

        let tag = tag.unwrap();
        let tag_local_index = tag_local_index.unwrap();
        let mut found = false;
        let members = tag.members();

        // Find the index of the member within the members of this tag and remove it
        for (i, node_index) in members.iter().enumerate() {
            if *node_index == inode.index() {
                found = true;
                self.tags[tag_local_index].remove_member_at(i as u16);

                // Write the tag
                self.write_to_address(
                    self.tag_index_to_address(self.tags[tag_local_index].index()),
                    &self.tags[tag_local_index].to_bytes().to_vec(),
                )?;

                break;
            }
        }

        // If we couldn't find it check indirect blocks
        if !found {
            let mut next = tag.indirect_pointer();
            let mut parent: Option<IndirectTagBlock> = None;
            let mut parent_address = None;

            while !found && next.is_some() {
                // Read the next indirect block
                let address = next.unwrap();
                let bytes = self.read_from_address(address, self.block_size)?;

                let mut block = match IndirectTagBlock::from_bytes(&bytes) {
                    Some(b) => b,
                    None => return Err(VoxFSError::CorruptedIndirectTag),
                };

                let members = block.members();

                // Parse the members and remove the target member if found
                for (i, member) in members.iter().enumerate() {
                    if *member == inode.index() {
                        block.remove_member_at(i as u16);
                        found = true;
                        break;
                    }
                }

                if found {
                    // Check if we need to delete this block if it has no members
                    if prune && block.number_of_members() == 0 {
                        // Here we remove any reference to this block, if this block points to another block
                        // we just adjust what points to the block to point to that block.
                        let index = self.address_to_data_index(address);

                        match parent {
                            Some(ref p) => {
                                // Copy the parent block
                                let mut cp = p.clone();
                                // Change it so we are are no longer pointing to the block we are deleting
                                cp.set_next_optional(block.next());
                                // Mark this empty block as non-existent
                                self.block_bitmap.set_bit(index as usize, false);

                                // Write the bitmaps
                                self.write_bitmaps()?;
                                // Write the parent block overtop the old one
                                self.write_to_address(
                                    parent_address.unwrap(),
                                    &cp.to_bytes_padded(self.block_size as usize),
                                )?;
                            }
                            None => {
                                // If the parent is None this means it was the actual tag block
                                // not an indirect block.

                                // So tell the tag to skip this block
                                self.tags[tag_local_index].set_indirect_optional(block.next());
                                // Mark this empty block as non-existent
                                self.block_bitmap.set_bit(index as usize, false);
                                // Write the bitmaps
                                self.write_bitmaps()?;

                                // Update the tag block with the new details
                                self.write_to_address(
                                    self.tag_index_to_address(self.tags[tag_local_index].index()),
                                    &self.tags[tag_local_index].to_bytes().to_vec(),
                                )?;
                            }
                        }
                    } else {
                        // Otherwise just update this block
                        self.write_to_address(
                            address,
                            &block.to_bytes_padded(self.block_size as usize),
                        )?;
                    }
                } else {
                    // If we didn't find the block set the parent for the next indirect block to be the
                    // one we just checked. Then point to the next block in the chain.
                    parent_address = next;
                    next = block.next();
                    parent = Some(block);
                }
            }

            if !found {
                return Err(VoxFSError::TagNotAppliedToINode);
            }
        }

        return Ok(());
    }

    /// List the inodes on the disk, that are members of a tag
    pub fn list_nodes_with_tag(&self, tag_index: u64) -> Result<Vec<INode>, VoxFSError<E>> {
        let mut tag = None;

        // Locate the tag in the memory map
        for t in &self.tags {
            if t.index() == tag_index {
                tag = Some(t);
                break;
            }
        }

        // Ensure we found it
        if tag.is_none() {
            return Err(VoxFSError::CouldNotFindTag);
        }

        return self.load_nodes_with_tag(&tag.unwrap());
    }

    /// List the inodes on the disk, that are members of a tag
    fn load_nodes_with_tag(&self, tag: &TagBlock) -> Result<Vec<INode>, VoxFSError<E>> {
        let mut nodes = Vec::new();

        match tag.indirect_pointer() {
            Some(indirect_address) => {
                let mut number_of_pointers = tag.number_of_pointers() as usize;
                let mut members = tag.members()[..number_of_pointers].to_vec();

                // Locate the member nodes and store them from the tag
                for node in &self.inodes {
                    // Break if we found everything
                    if number_of_pointers == 0 {
                        break;
                    }

                    for i in 0..number_of_pointers {
                        if node.index() == members[i] {
                            nodes.push(*node);
                            members.remove(i);
                            number_of_pointers -= 1;
                            break;
                        }
                    }
                }

                let mut next_address = Some(indirect_address);

                // Process the indirect blocks
                while next_address.is_some() {
                    // Read the block and check it
                    let bytes = self.read_from_address(next_address.unwrap(), self.block_size)?;
                    let block = match IndirectTagBlock::from_bytes(&bytes) {
                        Some(b) => b,
                        None => return Err(VoxFSError::CorruptedIndirectTag),
                    };

                    let mut block_members = block.members();

                    // Process the members by locating the loaded inodes in the memory map
                    for node in &self.inodes {
                        if block_members.len() == 0 {
                            break;
                        }

                        for i in 0..block_members.len() {
                            if node.index() == block_members[i] {
                                nodes.push(*node);
                                block_members.remove(i);
                                break;
                            }
                        }
                    }

                    next_address = block.next();
                }
            }
            None => {
                // If there was no indirect tag block
                let mut number_of_pointers = tag.number_of_pointers() as usize;
                let mut members = tag.members()[..number_of_pointers].to_vec();

                // Just push the inodes.
                // Locate the member nodes and store them
                for node in &self.inodes {
                    // Break if we found everything
                    if number_of_pointers == 0 {
                        break;
                    }

                    for i in 0..number_of_pointers {
                        if node.index() == members[i] {
                            nodes.push(*node);
                            members.remove(i);
                            number_of_pointers -= 1;
                            break;
                        }
                    }
                }
            }
        }

        return Ok(nodes);
    }

    /// List the inodes on the disk, that are members of the tags
    pub fn list_nodes_with_tags(&self, tag_indices: Vec<u64>) -> Result<Vec<INode>, VoxFSError<E>> {
        let mut tags = Vec::new();
        let mut tag_indices_cp = tag_indices.clone();

        // Locate the tags in the memory map
        for t in &self.tags {
            let i = tag_indices_cp.iter().position(|&e| e == t.index());
            if i.is_some() {
                tags.push(t);
                tag_indices_cp.remove(i.unwrap());
            }

            if tag_indices_cp.is_empty() {
                break;
            }
        }

        // Ensure we found all tags
        if !tag_indices_cp.is_empty() {
            return Err(VoxFSError::CouldNotFindTag);
        }

        let mut nodes = Vec::new();

        for tag in tags {
            let tag_nodes = self.load_nodes_with_tag(tag)?;

            if nodes.is_empty() {
                nodes = tag_nodes;
            } else {
                let mut n = 0;

                while nodes.len() > 0 && n < nodes.len() {
                    if !tag_nodes.contains(&nodes[n]) {
                        nodes.remove(n);
                    } else {
                        n += 1;
                    }
                }
            }

            if nodes.is_empty() {
                return Ok(nodes); // If this is ever reached then no inode was in all the tags so just return
            }
        }

        return Ok(nodes);
    }

    /// Gets the indices of tags based on their names.
    pub fn tags_with_names(&self, mut names: Vec<String>) -> Result<Vec<u64>, VoxFSError<E>> {
        if names.len() > self.tags.len() {
            return Err(VoxFSError::MoreNamesThanTagsProvided);
        }

        let mut indices = Vec::new();

        for tag in &self.tags {
            if names.len() == 0 {
                break;
            }

            let index = names.iter().position(|i| tag.same_name(&i));

            match index {
                Some(i) => {
                    indices.push(tag.index());
                    names.remove(i);
                }
                None => (),
            }
        }

        if names.len() > 0 {
            return Err(VoxFSError::NoTagsWithNames(names));
        }

        return Ok(indices);
    }

    /// Creates a new file in the first available index in the first available INode location.
    /// A copy of the inode is returned but the original is stored in the disk.
    pub fn create_new_file(
        &mut self,
        name: &str,
        flags: INodeFlags,
        contents: Vec<u8>,
    ) -> Result<INode, VoxFSError<E>> {
        self.validate_name(name, VoxFSError::InvalidFileName)?;

        // Find a free space to store the inode on the disk
        let inode_index = match self
            .inode_bitmap
            .find_next_0_index_up_to(self.super_block.inode_count() as usize)
        {
            Some(index) => index,
            None => return Err(VoxFSError::NoFreeInode),
        };

        // Request enough blocks to cover the size of the file
        let extents = match self.find_blocks(contents.len() as u64) {
            Some(extents) => extents,
            None => return Err(VoxFSError::NotEnoughFreeDataBlocks),
        };

        let mut contents_offset = 0;

        // We do this in two separate loops to prevent corrupting the memory bitmap, if something
        // was marked incorrectly.
        for (start, end) in &extents {
            for i in *start..=*end {
                if self.block_bitmap.bit_at(i as usize).unwrap() {
                    return Err(VoxFSError::BlockAlreadyAllocated);
                }
            }
        }

        for (start, end) in &extents {
            // Mark each block as taken
            for i in *start..=*end {
                self.block_bitmap.set_bit(i as usize, true);

                // Write the content for this block
                if contents_offset + self.block_size > contents.len() as u64 {
                    self.write_to_address(
                        self.data_index_to_address(i),
                        &contents[contents_offset as usize..].to_vec(),
                    )?;
                } else {
                    // We don't need to check if we have written all the data because we assume
                    // that we will be given the minimum blocks required so that once this else is reached
                    // it will be the last block
                    self.write_to_address(
                        self.data_index_to_address(i),
                        &contents[contents_offset as usize
                            ..(contents_offset + self.block_size) as usize]
                            .to_vec(),
                    )?;
                }

                // Increase the offset to account for the data written
                contents_offset += self.block_size;
            }
        }

        let inode;

        if extents.len() > 5 {
            let mut inode_extents = [Extent::zeroed(); 5];

            for i in 0..5 {
                inode_extents[i].start = extents[i].0;
                inode_extents[i].end = extents[i].1;
            }

            let mut indirects_addresses = Vec::new();

            let amount_per_indirect =
                IndirectINode::max_extents_for_blocksize(self.block_size) as usize;
            for mut i in 5..extents.len() {
                let mut block_indirects = Vec::new();

                while block_indirects.len() < amount_per_indirect && i < extents.len() {
                    block_indirects.push(extents[i]);
                    i += 1;
                }

                indirects_addresses.push(block_indirects);
            }

            let mut previous_address = 0;

            for address_group in indirects_addresses.iter().rev() {
                // Find a block
                let block_index = match self
                    .block_bitmap
                    .find_next_0_index_up_to(self.super_block.block_count() as usize)
                {
                    Some(b) => b as u64,
                    None => return Err(VoxFSError::NotEnoughFreeDataBlocks),
                };

                let address = self.data_index_to_address(block_index);
                let block = IndirectINode::new(
                    address_group
                        .iter()
                        .map(|b| Extent {
                            start: b.0,
                            end: b.1,
                        })
                        .collect(),
                    previous_address,
                    self.block_size,
                );

                self.write_to_address(address, &block.to_bytes())?;
                previous_address = address;
                self.block_bitmap.set_bit(block_index as usize, true);
            }

            let current_time = self.manager.current_time();

            inode = INode::new(
                inode_index as u64,
                name,
                contents.len() as u64,
                flags,
                current_time,
                current_time,
                current_time,
                previous_address,
                extents.len() as u8,
                inode_extents,
            );
        } else {
            let mut extent_blocks = [Extent::zeroed(); 5];

            for i in 0..(if extents.len() < 5 { extents.len() } else { 5 }) {
                extent_blocks[i] = Extent {
                    start: extents[i].0,
                    end: extents[i].1,
                };
            }

            let current_time = self.manager.current_time();
            inode = INode::new(
                inode_index as u64,
                name,
                contents.len() as u64,
                flags,
                current_time,
                current_time,
                current_time,
                0,
                extents.len() as u8,
                extent_blocks,
            );
        }

        self.write_to_address(
            self.inode_index_to_address(inode_index as u64),
            &inode.to_bytes().to_vec(),
        )?;

        if !self.inode_bitmap.set_bit(inode_index, true) {
            panic!("Unexpected fail."); // This should never happen but if it does then its a developer error so panic.
        }

        self.write_bitmaps()?;

        self.inodes.push(inode);

        return Ok(inode);
    }

    /// Returns the approximate file size of an inode.
    /// This method is approximate only because it rounds up based on the file size to the nearest block,
    /// instead of measuring the size of each extent. This method does not read from the disk.
    pub fn approximate_file_size(&self, inode_index: u64) -> Result<u64, VoxFSError<E>> {
        for inode in &self.inodes {
            if inode.index() == inode_index {
                return Ok(
                    inode.file_size() + (self.block_size - (inode.file_size() % self.block_size))
                );
            }
        }

        return Err(VoxFSError::CouldNotFindINode);
    }

    /// Returns the actual file size and the physical on disk file size.
    /// This method does read from the disk.
    pub fn file_size(&self, inode_index: u64) -> Result<FileSize, VoxFSError<E>> {
        // Locate the inode
        let inode = self.inodes[self.locate_inode(inode_index)?];

        let actual_size = inode.file_size();
        let mut physical_size = 0;
        let mut next = inode.indirect_pointer();

        // Calculate the size of each extent and add it to the overall physical size
        for i in 0..inode.num_extents() as usize {
            let extent = inode.blocks()[i];
            physical_size += (extent.end - extent.start + 1) * self.block_size; // +1 because inclusive
        }

        while next.is_some() {
            let bytes = self.read_from_address(next.unwrap(), self.block_size)?;
            let indirect_inode = match IndirectINode::from_bytes(&bytes) {
                Some(i) => i,
                None => return Err(VoxFSError::CorruptedIndirectINode),
            };

            for extent in &indirect_inode.extents() {
                physical_size += (extent.end - extent.start + 1) * self.block_size;
                // +1 because inclusive
            }

            next = indirect_inode.next();
        }

        return Ok(FileSize {
            actual_size,
            physical_size,
        });
    }

    /// Reads an entire file from the disk
    pub fn read_file(&self, inode_index: u64) -> Result<Vec<u8>, VoxFSError<E>> {
        return self.read_file_bytes(inode_index, 0);
    }

    /// Reads a specified amount of bytes from the start of a file. If num_bytes is greater than the length of the file or num_bytes == 0, only up to the size of the file will be returned.
    pub fn read_file_bytes(
        &self,
        inode_index: u64,
        num_bytes: u64,
    ) -> Result<Vec<u8>, VoxFSError<E>> {
        // Locate the INode object in the memory map
        let inode = self.inodes[self.locate_inode(inode_index)?];

        let mut result_bytes = Vec::new();

        let num_bytes = {
            if num_bytes > inode.file_size() {
                inode.file_size()
            } else if num_bytes == 0 {
                inode.file_size()
            } else {
                num_bytes
            }
        };

        // Whilst it is possible to read large chunks, for the sake of simplicity for the driver implementor,
        // we will not read amounts larger than the block size.

        let mut amount_read = 0;
        let blocks = inode.blocks();

        for extent_index in 0..inode.num_extents() as usize {
            let extent = blocks[extent_index];

            // Read the data in the extent
            let mut content = self.read_extent(extent)?;

            // Check if we need to add all the data from the extent or just some of it
            if content.len() + result_bytes.len() > num_bytes as usize {
                // We don't need all the data we read to slice it to the amount we need.
                let bytes_to_read = num_bytes - result_bytes.len() as u64;
                result_bytes.extend_from_slice(&content[..bytes_to_read as usize]);
                amount_read += bytes_to_read;
            } else {
                // We need all the data from the extent so store it
                let bytes_read = content.len();
                result_bytes.append(&mut content);
                amount_read += bytes_read as u64;
            }
        }

        // Now we need to read from any indirect
        let mut next = inode.indirect_pointer();

        // If there is data still to read iterate over the indirect blocks
        while amount_read < num_bytes {
            // If we don't have a new indirect block to read then throw an error.
            if next.is_none() {
                return Err(VoxFSError::ExpectedIndirectNode);
            }

            // Read the indirect inode
            let bytes = self.read_from_address(next.unwrap(), self.block_size)?;
            let indirect = match IndirectINode::from_bytes(&bytes) {
                Some(i) => i,
                None => return Err(VoxFSError::CorruptedIndirectINode),
            };

            for extent in &indirect.extents() {
                let mut content = self.read_extent(*extent)?;

                // Check if we need to add all the data from the extent or just some of it
                if content.len() + result_bytes.len() > num_bytes as usize {
                    // We don't need all the data we read to slice it to the amount we need.
                    let bytes_to_read = num_bytes - result_bytes.len() as u64;
                    result_bytes.extend_from_slice(&content[..bytes_to_read as usize]);
                    amount_read += bytes_to_read;
                } else {
                    // We need all the data from the extent so store it
                    let bytes_read = content.len();
                    result_bytes.append(&mut content);
                    amount_read += bytes_read as u64;
                }
            }

            next = indirect.next();
        }

        return Ok(result_bytes);
    }

    /// Appends bytes to a file.
    pub fn append_file_bytes(
        &mut self,
        inode_index: u64,
        bytes: &Vec<u8>,
    ) -> Result<(), VoxFSError<E>> {
        // locate the inode in our memory map
        let mut inode_local_index = None;

        for (i, node) in self.inodes.iter().enumerate() {
            if node.index() == inode_index {
                inode_local_index = Some(i);
                break;
            }
        }

        // Ensure we actually found an inode
        if inode_local_index.is_none() {
            return Err(VoxFSError::CouldNotFindINode);
        }

        let inode_local_index = inode_local_index.unwrap();
        let inode = self.inodes[inode_local_index];

        // Find the last extent and how much space of that extent is available.
        let mut last_block_extent = inode.blocks()[(inode.num_extents() - 1) as usize];
        let mut next = inode.indirect_pointer();
        let mut previous = None;

        while next.is_some() {
            // Read the next indirect inode
            let bytes = self.read_from_address(next.unwrap(), self.block_size)?;
            let indirect_inode = match IndirectINode::from_bytes(&bytes) {
                Some(i) => i,
                None => return Err(VoxFSError::CorruptedIndirectINode),
            };

            // If this is the last indirect node mark the last extent so we check how much of it is left
            // when we append
            if indirect_inode.next().is_none() {
                last_block_extent = indirect_inode.last_extent().unwrap();
            }

            previous = next;
            next = indirect_inode.next();
        }

        // Check how much space is left in that last block
        let amount_available = self.block_size - (inode.file_size() % self.block_size);

        if amount_available > bytes.len() as u64 {
            // If we can fit all the required data into the space that's available just do that.
            let block_address = self.data_index_to_address(last_block_extent.end);
            let address = block_address + (self.block_size - amount_available);

            // Write as many bytes as we can
            self.write_to_address(address, &bytes)?;

            // Update the inode to reflect the new size
            self.inodes[inode_local_index].increase_file_size(bytes.len() as u64);
            self.write_to_address(
                self.inode_index_to_address(self.inodes[inode_local_index].index()),
                &self.inodes[inode_local_index].to_bytes().to_vec(),
            )?;
        } else {
            // This could potentially be improved by checking the already existing extents for space either side but I don't see the practical advantage in the long term to this approach.

            // Otherwise we need to allocate more data blocks
            let block_address = self.data_index_to_address(last_block_extent.end);
            let address = block_address + (self.block_size - amount_available);

            // Finds data blocks
            let pointers = match self.find_blocks(bytes.len() as u64 - amount_available) {
                Some(p) => p,
                None => return Err(VoxFSError::NotEnoughFreeDataBlocks),
            };

            // Make sure each block is free. We do this just to double check we
            // don't overwrite anything. We separate this from the next loop since
            // we could fail to set a bit and not be able to undo.
            for (start, end) in &pointers {
                for i in *start..=*end {
                    if self.block_bitmap.bit_at(i as usize).unwrap() {
                        return Err(VoxFSError::BlockAlreadyAllocated);
                    }
                }
            }

            let mut extents = Vec::new();

            // Mark each block as taken
            for (start, end) in &pointers {
                for i in *start..=*end {
                    if !self.block_bitmap.set_bit(i as usize, true) {
                        return Err(VoxFSError::FailedToSetBitmapBit);
                    }
                }

                extents.push(Extent {
                    start: *start,
                    end: *end,
                });
            }

            // This tracks how many extents we still need to add to a node
            let mut remaining = extents.len();

            // Append as many extents as possible to the root inode
            while self.inodes[inode_local_index].num_extents() < INode::max_extents()
                && remaining > 0
            {
                if !self.inodes[inode_local_index].append_extent(extents[extents.len() - remaining])
                {
                    panic!("Unexpected fail. Description: Failed to append extent to an inode");
                    // This should never be reached.
                }

                remaining -= 1;
            }

            // Check if an indirect node already exists and append to that node instead
            next = inode.indirect_pointer();

            while next.is_some() && remaining > 0 {
                // Read the indirect inode
                let bytes = self.read_from_address(next.unwrap(), self.block_size)?;

                let mut indirect_inode = match IndirectINode::from_bytes(&bytes) {
                    Some(i) => i,
                    None => return Err(VoxFSError::CorruptedIndirectINode),
                };

                // Tell the inode how many extents it can hold
                indirect_inode.set_maximum_extents_blocksize(self.block_size);

                // If we changed the indirect then we need to write it
                let mut changed_indirect = false;

                // Keep appending extents while we can
                while remaining > 0
                    && indirect_inode.append_extent(extents[extents.len() - remaining])
                {
                    remaining -= 1;
                    changed_indirect = true;
                }

                if changed_indirect {
                    self.write_to_address(next.unwrap(), &indirect_inode.to_bytes())?;
                }

                next = indirect_inode.next();
            }

            // If we couldn't append all the extents create a new indirectinode
            if remaining > 0 {
                // Create the new indirect
                let new_indirect = IndirectINode::new(
                    extents[extents.len() - remaining..].to_vec(),
                    0,
                    self.block_size,
                );

                // Locate a spot for it
                let index = match self.find_block() {
                    Some(i) => i,
                    None => return Err(VoxFSError::NotEnoughFreeDataBlocks),
                };

                let indirect_address = self.data_index_to_address(index);

                // Ensure the block was free
                if self.block_bitmap.bit_at(index as usize).unwrap() {
                    return Err(VoxFSError::BlockAlreadyAllocated);
                }

                // Mark it as taken
                if !self.block_bitmap.set_bit(index as usize, true) {
                    return Err(VoxFSError::FailedToSetBitmapBit);
                }

                // We need to set a pointer to this new indirect block
                match previous {
                    Some(a) => {
                        let previous_bytes = self.read_from_address(a, self.block_size)?;
                        let mut previous_indirect = match IndirectINode::from_bytes(&previous_bytes)
                        {
                            Some(i) => i,
                            None => return Err(VoxFSError::CorruptedIndirectINode),
                        };

                        // Update the indirect pointer to point to this new indirect
                        previous_indirect.set_next(indirect_address);
                        self.write_to_address(a, &previous_indirect.to_bytes())?;
                    }
                    None => {
                        // Add to the original inode a pointer to this new indirect
                        self.inodes[inode_local_index].set_indirect_pointer(Some(indirect_address));
                        // NOTE: We don't write to the disk here because we will write this inode at the end anyway
                    }
                }

                // Write the new indirect block
                self.write_to_address(indirect_address, &new_indirect.to_bytes())?;
            }

            // Write as many bytes to the last block as possible
            self.write_to_address(address, &bytes[..amount_available as usize].to_vec())?;

            // Continue on and write to each of the new extents
            let mut offset = 0;
            for extent in extents {
                for index in extent.start..=extent.end {
                    let index_address = self.data_index_to_address(index);
                    let bytes_end_index = amount_available + ((offset + 1) * self.block_size); // Add 1 to the offset to account for the fact we want to write block_size

                    if bytes_end_index >= bytes.len() as u64 {
                        self.write_to_address(
                            index_address,
                            &bytes[amount_available as usize..].to_vec(),
                        )?;
                    } else {
                        self.write_to_address(
                            index_address,
                            &bytes[amount_available as usize..bytes_end_index as usize].to_vec(),
                        )?;
                    }

                    offset += 1;
                }
            }

            self.inodes[inode_local_index].increase_file_size(bytes.len() as u64);
            self.write_to_address(
                self.inode_index_to_address(self.inodes[inode_local_index].index()),
                &self.inodes[inode_local_index].to_bytes().to_vec(),
            )?;
        }

        return Ok(());
    }

    /// Deletes a file.
    pub fn delete_file(&mut self, inode_index: u64) -> Result<(), VoxFSError<E>> {
        let local_index = self.locate_inode(inode_index)?;
        let inode = self.inodes[local_index];

        let mut next = inode.indirect_pointer();
        let mut extents = Vec::new();
        let mut indirect_indexes = Vec::new();

        while next.is_some() {
            // Read each indirect in
            let bytes = self.read_from_address(next.unwrap(), self.block_size)?;
            let indirect = match IndirectINode::from_bytes(&bytes) {
                Some(i) => i,
                None => return Err(VoxFSError::CorruptedIndirectINode),
            };

            // Since next is an address we need a data block index
            let data_block_index = self.address_to_data_index(next.unwrap());
            let mut ind_extents = indirect.extents();

            // Track the extents and indirect block indices to mark as free
            extents.append(&mut ind_extents);
            indirect_indexes.push(data_block_index);

            next = indirect.next();
        }

        extents.extend_from_slice(&inode.blocks());

        // We need to ensure this inode isn't being pointed to by any tags.

        // I don't know a more efficient method of doing this :/
        let indices: Vec<u64> = self.tags.iter().map(|t| t.index()).collect();
        for index in indices {
            match self.remove_tag_from_inode(index, inode.index()) {
                Ok(_) => (),
                Err(e) => match e {
                    VoxFSError::TagNotAppliedToINode => (),
                    _ => return Err(e),
                },
            }
        }

        // Mark each indirect index as free
        for index in indirect_indexes {
            if !self.block_bitmap.set_bit(index as usize, false) {
                return Err(VoxFSError::FailedToFreeBlock);
            }
        }

        // Mark the blocks in each extent as free
        for extent in &extents {
            for i in extent.start..=extent.end {
                if !self.block_bitmap.set_bit(i as usize, false) {
                    return Err(VoxFSError::FailedToFreeBlock);
                }
            }
        }

        // Mark the inode as free
        if !self.inode_bitmap.set_bit(inode.index() as usize, false) {
            return Err(VoxFSError::FailedToFreeINode);
        }

        // Remove it from the memory map
        self.inodes.remove(local_index);

        // Update the disk
        self.write_bitmaps()?;

        return Ok(());
    }

    /// Writes the block availability bit maps
    fn write_bitmaps(&mut self) -> Result<(), VoxFSError<E>> {
        // Write the tags bitmap
        self.write_to_address(self.block_size, &self.tag_bitmap.as_bytes())?; // We start at blocksize because of the superblock

        // Write the inodes bitmap
        self.write_to_address(
            self.block_size + self.blocks_for_tag_map * self.block_size,
            &self.inode_bitmap.as_bytes(),
        )?; // We start at blocksize because of the superblock then skip the tag map

        // Write the data bitmap
        self.write_to_address(
            self.block_size
                + (self.blocks_for_tag_map + self.blocks_for_inode_map) * self.block_size,
            &self.block_bitmap.as_bytes(),
        )?; // We start at blocksize because of the superblock then skip the tag map and inode map

        return Ok(());
    }

    /// Stores a new tag in the first available spot. It will set the index field of the tag block
    fn store_tag_first_free(&mut self, mut tag: TagBlock) -> Result<TagBlock, VoxFSError<E>> {
        // Find the first available index and load it
        let index = match self
            .tag_bitmap
            .find_next_0_index_up_to(self.super_block.tag_count() as usize)
        {
            Some(index) => index,
            None => return Err(VoxFSError::NoFreeInode),
        };

        // Set the index in the tag
        tag.set_index(index as u64);

        // Write the tag to the disk
        self.write_to_address(
            self.tag_index_to_address(index as u64),
            &tag.to_bytes().to_vec(),
        )?;

        // Set the spot as taken
        if !self.tag_bitmap.set_bit(index, true) {
            panic!("Unexpected fail."); // This should never happen but if it does then its a developer error so panic.
        }

        // Write the bitmap
        self.write_bitmaps()?;

        return Ok(tag);
    }

    /// Load a list of the tags from the disk
    fn load_tags(&self) -> Result<Vec<TagBlock>, VoxFSError<E>> {
        let mut tags = Vec::new();

        for i in 0..self.super_block.tag_count() {
            // We unwrap here because we assume the bit exists
            if self.tag_bitmap.bit_at(i as usize).unwrap() {
                // Read the tag and add it to the memory map
                let location = self.tag_index_to_address(i);
                let bytes = self.read_from_address(location, TagBlock::size())?;

                // Ensure the tag isn't corrupted
                tags.push(match TagBlock::from_bytes(&bytes) {
                    Some(tag) => tag,
                    None => return Err(VoxFSError::CorruptedTag),
                });
            }
        }

        return Ok(tags);
    }

    /// Reads and loads all the inodes on the filesystem.
    fn load_inodes(&mut self) -> Result<Vec<INode>, VoxFSError<E>> {
        let mut inodes: Vec<INode> = Vec::new();

        for i in 0..self.super_block.inode_count() {
            // If this bit is marked as taken then read the inode at that location
            if self.inode_bitmap.bit_at(i as usize).unwrap() {
                // Read the inode and add it to the memory map
                let address = self.inode_index_to_address(i);
                let bytes = self.read_from_address(address, INode::size())?;

                // Ensure the INode was valid
                inodes.push(match INode::from_bytes(&bytes) {
                    Some(node) => node,
                    None => return Err(VoxFSError::CorruptedINode),
                });
            }
        }

        return Ok(inodes);
    }

    /// Locates extents and returns a vector of tuples where .0 is the start address and .1 is the end address.
    /// min_size: The minimum size needed in BYTES
    fn find_blocks(&self, min_size: u64) -> Option<Vec<(u64, u64)>> {
        // Calculate how many blocks we need for the minimum size
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

        let mut first_index = self
            .block_bitmap
            .find_next_0_index_up_to(self.super_block.block_count() as usize)
            .unwrap() as u64;

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
                // Check if we found an extent big enough
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

    /// Locates a single available data block.
    fn find_block(&self) -> Option<u64> {
        // Check if we have enough blocks.
        if (self
            .block_bitmap
            .count_zeros_up_to(self.super_block.block_count() as usize))
        .unwrap()
            < 1
        {
            return None;
        }

        // Otherwise return the first available index
        return match self
            .block_bitmap
            .find_next_0_index_up_to(self.super_block.block_count() as usize)
        {
            Some(v) => Some(v as u64),
            None => None,
        };
    }

    /// Converts an address into a data block index, panics if not possible.
    #[inline]
    fn address_to_data_index(&self, address: u64) -> u64 {
        assert!(
            address > self.super_block.data_start_address(),
            "Invalid address conversion requested."
        );

        return (address - self.super_block.data_start_address()) / self.block_size;
    }

    /// Converts data block index into an address
    #[inline]
    fn data_index_to_address(&self, index: u64) -> u64 {
        return self.super_block.data_start_address() + index * self.block_size;
    }

    /// Converts inode block index into an address
    #[inline]
    fn inode_index_to_address(&self, index: u64) -> u64 {
        return self.super_block.inode_start_address() + index * INode::size();
    }

    /// Converts tag block index into an address
    #[inline]
    fn tag_index_to_address(&self, index: u64) -> u64 {
        return self.super_block.tag_start_address() + index * TagBlock::size();
    }

    /// Write data to an address on the disk
    #[inline]
    fn write_to_address(&mut self, address: u64, content: &Vec<u8>) -> Result<(), VoxFSError<E>> {
        match self.handler.write_bytes(content, address) {
            Ok(_) => return Ok(()),
            Err(e) => return Err(e.into_voxfs_error()),
        }
    }

    /// Read data from an address on the disk.
    #[inline]
    fn read_from_address(
        &self,
        address: u64,
        number_of_bytes: u64,
    ) -> Result<Vec<u8>, VoxFSError<E>> {
        match self.handler.read_bytes(address, number_of_bytes) {
            Ok(b) => return Ok(b),
            Err(e) => return Err(e.into_voxfs_error()),
        }
    }

    /// Read blocks between two data indexes, INCLUSIVE at both ends
    fn read_between_range(&self, start: u64, end: u64) -> Result<Vec<u8>, VoxFSError<E>> {
        let mut result = Vec::new();

        for i in start..=end {
            let addr = self.data_index_to_address(i);
            let mut content = self.read_from_address(addr, self.block_size)?;
            result.append(&mut content);
        }

        return Ok(result);
    }

    /// Read data from an extent
    fn read_extent(&self, extent: Extent) -> Result<Vec<u8>, VoxFSError<E>> {
        return self.read_between_range(extent.start, extent.end);
    }

    /// Locates an inode based on an inode index, it returns the index in the memory map
    fn locate_inode(&self, inode_index: u64) -> Result<usize, VoxFSError<E>> {
        for i in 0..self.inodes.len() {
            if self.inodes[i].index() == inode_index {
                return Ok(i as usize);
            }
        }

        return Err(VoxFSError::CouldNotFindINode);
    }

    /// Checks if a tag/inode name contains any forbidden characters
    fn validate_name(&self, name: &str, err: VoxFSError<E>) -> Result<(), VoxFSError<E>> {
        for ref ch in name.chars() {
            if FORBIDDEN_CHARACTERS.contains(ch) {
                return Err(err);
            }
        }

        return Ok(());
    }
}
