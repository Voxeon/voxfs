use super::disk_blocks::SuperBlock;
use super::DiskHandler;
use crate::bitmap::BitMap;
use crate::disk::disk_blocks::{
    Extent, INode, INodeFlags, IndirectINode, IndirectTagBlock, TagBlock, TagFlags,
};
use crate::{ByteSerializable, OSManager, VoxFSError, VoxFSErrorConvertible};
use alloc::{vec, vec::Vec};

const DEFAULT_BLOCK_SIZE: u64 = 4_096; // In bytes. 4KiB.

// TODO List
// TODO: 1. Support deleting files
// - TODO: 2. Support deleting tags
// - TODO: 3. Support removing file from a tag
// - TODO: 4. Support appending to files.
// TODO: 5. Support overwriting files.

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
    pub block_bitmap: BitMap,

    pub block_size: u64,

    blocks_for_tag_map: u64,
    blocks_for_inode_map: u64,
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

    /// Deletes a tag
    pub fn delete_tag(&mut self, index: u64) -> Result<(), VoxFSError<E>> {
        let mut local_index = None; // The index of the tag in the vector in memory
        let mut local_tag = None;

        for (i, tag) in self.tags.iter().enumerate() {
            if tag.index() == index {
                local_index = Some(i);
                local_tag = Some(tag.clone());
                break;
            }
        }

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
            let bytes = unwrap_return_error_voxfs_convertible!(self
                .handler
                .read_bytes(address, self.block_size));

            let block = match IndirectTagBlock::from_bytes(&bytes) {
                Some(b) => b,
                None => return Err(VoxFSError::CorruptedIndirectTag),
            };

            current_indirect = block.next();
            data_block_indices.push(index);
        }

        // Mark the space as free
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

    /// Add an inode to a tag
    pub fn apply_tag(&mut self, tag_index: u64, inode: &INode) -> Result<(), VoxFSError<E>> {
        // Locate the tag in the memory map from the disk index provided
        let mut tag_self_index = None;

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

        // This checks if we have enough space in the tagblock itself to add a new member
        // if not we create a new indirect tag block
        if self.tags[tag_self_index].number_of_pointers() >= TagBlock::MAXIMUM_LOCAL_MEMBERS {
            /*
            Two things must be checked.
            1. Has this tag been applied before to this INode.
            2. Where is a free spot?
             */

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
                let bytes = unwrap_return_error_voxfs_convertible!(self
                    .handler
                    .read_bytes(next_address.unwrap(), self.block_size));

                let indirect_tag = match IndirectTagBlock::from_bytes(&bytes) {
                    Some(t) => t,
                    None => return Err(VoxFSError::CorruptedIndirectTag),
                };

                // Check if we contain thee member already
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
                unwrap_return_error_voxfs_convertible!(self.handler.write_bytes(
                    &indirect_tag.to_bytes_padded(self.block_size as usize),
                    location
                ));

                // Write the bitmaps
                self.write_bitmaps()?;

                // We need to point to this new block somehow...
                // Set the previous indirect block to point to this or the root tag
                match previous_indirect {
                    Some(mut i) => {
                        i.set_next(location);

                        unwrap_return_error_voxfs_convertible!(self.handler.write_bytes(
                            &i.to_bytes_padded(self.block_size as usize),
                            previous_indirect_location.unwrap()
                        ));
                    }
                    None => {
                        self.tags[tag_self_index].set_indirect(location);

                        unwrap_return_error_voxfs_convertible!(self.handler.write_bytes(
                            &self.tags[tag_self_index].to_bytes().to_vec(),
                            self.super_block.tag_start_address()
                                + self.tags[tag_self_index].index() * TagBlock::size()
                        ));
                    }
                }
            } else {
                // Otherwise place it in the free spot

                // Read the indirect block and check that it was valid
                let mut indirect = match IndirectTagBlock::from_bytes(
                    &unwrap_return_error_voxfs_convertible!(self
                        .handler
                        .read_bytes(free_indirect_tag_address.unwrap(), self.block_size)),
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
                unwrap_return_error_voxfs_convertible!(self.handler.write_bytes(
                    &indirect.to_bytes_padded(self.block_size as usize),
                    free_indirect_tag_address.unwrap()
                ));
            }
        } else {
            // There's enough space to add it to the tag block itself

            // Check if this tag has already been applied.
            if self.tags[tag_self_index].contains_member(&inode.index()) {
                return Err(VoxFSError::TagAlreadyAppliedToINode);
            }

            self.tags[tag_self_index].append_member(inode.index());

            unwrap_return_error_voxfs_convertible!(self.handler.write_bytes(
                &self.tags[tag_self_index].to_bytes().to_vec(),
                self.super_block.tag_start_address()
                    + self.tags[tag_self_index].index() * TagBlock::size()
            ));
        }

        return Ok(());
    }

    /// Remove a tag from an inode, automatically deleting an empty indirect tag block.
    pub fn remove_tag_from_inode(
        &mut self,
        tag_index: u64,
        inode: &INode,
    ) -> Result<(), VoxFSError<E>> {
        return self.remove_tag_from_inode_optional_prune(tag_index, inode, true);
    }

    /// Remove a tag from an inode, if prune is true an empty indirect tag block is deleted.
    pub fn remove_tag_from_inode_optional_prune(
        &mut self,
        tag_index: u64,
        inode: &INode,
        prune: bool,
    ) -> Result<(), VoxFSError<E>> {
        let mut tag = None;
        let mut tag_local_index = None;

        for (i, t) in self.tags.iter().enumerate() {
            if t.index() == tag_index {
                tag = Some(*t);
                tag_local_index = Some(i);
            }
        }

        if tag.is_none() {
            return Err(VoxFSError::CouldNotFindTag);
        }

        let tag = tag.unwrap();
        let tag_local_index = tag_local_index.unwrap();
        let mut found = false;
        let members = tag.members();

        for (i, node_index) in members.iter().enumerate() {
            if *node_index == inode.index() {
                found = true;
                self.tags[tag_local_index].remove_member_at(i as u16);
                break;
            }
        }

        if !found {
            let mut next = tag.indirect_pointer();
            let mut parent: Option<IndirectTagBlock> = None;
            let mut parent_address = None;

            while !found && next.is_some() {
                let address = next.unwrap();
                let bytes = unwrap_return_error_voxfs_convertible!(self
                    .handler
                    .read_bytes(address, self.block_size));
                let mut block = match IndirectTagBlock::from_bytes(&bytes) {
                    Some(b) => b,
                    None => return Err(VoxFSError::CorruptedIndirectTag),
                };

                let members = block.members();

                for (i, member) in members.iter().enumerate() {
                    if *member == inode.index() {
                        block.remove_member_at(i as u16);
                        found = true;
                        break;
                    }
                }

                if found {
                    if prune && block.number_of_members() == 0 {
                        // Here we remove any reference to this block, if this block points to another block we just adjust what points to the block to point to that block.
                        let index = self.address_to_data_index(address);

                        match parent {
                            Some(ref p) => {
                                let mut cp = p.clone();
                                cp.set_next_optional(block.next());
                                self.block_bitmap.set_bit(index as usize, false);

                                self.write_bitmaps()?;
                                unwrap_return_error_voxfs_convertible!(self.handler.write_bytes(
                                    &cp.to_bytes_padded(self.block_size as usize),
                                    parent_address.unwrap()
                                ));
                            }
                            None => {
                                self.tags[tag_local_index].set_indirect_optional(block.next());
                                self.block_bitmap.set_bit(index as usize, false);

                                self.write_bitmaps()?;
                                unwrap_return_error_voxfs_convertible!(self.handler.write_bytes(
                                    &self.tags[tag_local_index].to_bytes().to_vec(),
                                    self.super_block.tag_start_address()
                                        + self.tags[tag_local_index].index() * TagBlock::size()
                                ));

                                //unwrap_return_error_voxfs_convertible!(self.handler.write_bytes(&tag.to_bytes().to_vec(), self.super_block.tag_start_address() + tag.index() * TagBlock::size()));
                            }
                        }
                    } else {
                        unwrap_return_error_voxfs_convertible!(self.handler.write_bytes(
                            &block.to_bytes_padded(self.block_size as usize),
                            address
                        ));
                    }
                } else {
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
    pub fn list_tag_nodes(&self, tag_index: u64) -> Result<Vec<INode>, VoxFSError<E>> {
        let mut tag = None;

        for t in &self.tags {
            if t.index() == tag_index {
                tag = Some(t);
                break;
            }
        }

        if tag.is_none() {
            return Err(VoxFSError::CouldNotFindTag);
        }

        let tag = tag.unwrap();

        let mut nodes = Vec::new();

        match tag.indirect_pointer() {
            Some(indirect_address) => {
                let mut next_address = Some(indirect_address);
                let members = tag.members().to_vec();

                for i in 0..tag.number_of_pointers() {
                    let index = members[i as usize];

                    for node in &self.inodes {
                        if node.index() == index {
                            nodes.push(*node);
                        }
                    }
                }

                while next_address.is_some() {
                    let bytes = unwrap_return_error_voxfs_convertible!(self
                        .handler
                        .read_bytes(next_address.unwrap(), self.block_size));
                    let block = IndirectTagBlock::from_bytes(&bytes).unwrap();

                    let block_members = block.members();

                    for i in 0..block.number_of_members() as usize {
                        for node in &self.inodes {
                            if node.index() == block_members[i] {
                                nodes.push(*node);
                            }
                        }
                    }

                    next_address = block.next();
                }
            }
            None => {
                let members = tag.members();

                for i in 0..tag.number_of_pointers() {
                    let index = members[i as usize];

                    for node in &self.inodes {
                        if node.index() == index {
                            nodes.push(*node);
                        }
                    }
                }
            }
        }

        return Ok(nodes);
    }

    /// Creates a new file in the first available index in the first available INode location. A copy of the inode is returned but the original is stored in the disk.
    pub fn create_new_file(
        &mut self,
        name: &str,
        flags: INodeFlags,
        contents: Vec<u8>,
    ) -> Result<INode, VoxFSError<E>> {
        let inode_index = match self
            .inode_bitmap
            .find_next_0_index_up_to(self.super_block.inode_count() as usize)
        {
            Some(index) => index,
            None => return Err(VoxFSError::NoFreeInode),
        };

        let blocks = match self.find_blocks(contents.len() as u64) {
            Some(extents) => extents,
            None => return Err(VoxFSError::NotEnoughFreeDataBlocks),
        };

        let mut contents_offset = 0;

        // We do this in two separate loops to prevent corrupting the memory bitmap.
        for (start, end) in &blocks {
            for i in *start..=*end {
                if self.block_bitmap.bit_at(i as usize).unwrap() {
                    return Err(VoxFSError::BlockAlreadyAllocated);
                }
            }
        }

        for (start, end) in &blocks {
            for i in *start..=*end {
                self.block_bitmap.set_bit(i as usize, true);

                if contents_offset + self.block_size > contents.len() as u64 {
                    unwrap_return_error_voxfs_convertible!(self.handler.write_bytes(
                        &contents[contents_offset as usize..].to_vec(),
                        self.super_block.data_start_address() + i * self.block_size,
                    ));
                } else {
                    unwrap_return_error_voxfs_convertible!(self.handler.write_bytes(
                        &contents[contents_offset as usize
                            ..(contents_offset + self.block_size) as usize]
                            .to_vec(),
                        self.super_block.data_start_address() + i * self.block_size,
                    ));
                }

                contents_offset += self.block_size;
            }
        }

        let inode;

        if blocks.len() > 5 {
            let mut inode_extents = [Extent::zeroed(); 5];

            for i in 0..5 {
                inode_extents[i].start = blocks[i].0;
                inode_extents[i].end = blocks[i].1;
            }

            let mut indirects_addresses = Vec::new();

            let amount_per_indirect =
                IndirectINode::max_extents_for_blocksize(self.block_size) as usize;
            for mut i in 5..blocks.len() {
                let mut block_indirects = Vec::new();

                while block_indirects.len() < amount_per_indirect && i < blocks.len() {
                    block_indirects.push(blocks[i]);
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

                let address = self.super_block.data_start_address() + block_index * self.block_size;
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
                unwrap_return_error_voxfs_convertible!(self
                    .handler
                    .write_bytes(&block.to_bytes(), address));
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
                blocks.len() as u8,
                inode_extents,
            );
        } else {
            let mut extent_blocks = [Extent::zeroed(); 5];

            for i in 0..(if blocks.len() < 5 { blocks.len() } else { 5 }) {
                extent_blocks[i] = Extent {
                    start: blocks[i].0,
                    end: blocks[i].1,
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
                blocks.len() as u8,
                extent_blocks,
            );
        }

        unwrap_return_error_voxfs_convertible!(self.handler.write_bytes(
            &inode.to_bytes().to_vec(),
            self.super_block.inode_start_address() + (inode_index as u64) * INode::size()
        ));

        if !self.inode_bitmap.set_bit(inode_index, true) {
            panic!("Unexpected fail."); // This should never happen but if it does then its a developer error so panic.
        }

        self.write_bitmaps()?;

        self.inodes.push(inode);

        return Ok(inode);
    }

    /// Returns the actual file size and the physical on disk file size
    pub fn file_size(&self, inode: &INode) -> Result<FileSize, VoxFSError<E>> {
        let actual_size = inode.file_size();
        let mut physical_size = 0;
        let mut next = inode.indirect_pointer();

        for i in 0..inode.num_extents() as usize {
            let extent = inode.blocks()[i];
            physical_size += (extent.end - extent.start + 1) * self.block_size; // +1 because inclusive
        }

        while next.is_some() {
            let bytes = unwrap_return_error_voxfs_convertible!(self
                .handler
                .read_bytes(next.unwrap(), self.block_size));
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
        let mut inode = None;

        for node in &self.inodes {
            if node.index() == inode_index {
                inode = Some(*node);
                break;
            }
        }

        if inode.is_none() {
            return Err(VoxFSError::CouldNotFindINode);
        }

        let inode = inode.unwrap();

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

        // Whilst it is possible to read large chunks, for the sake of simplicity for the driver implementor, we will not read amounts larger than the block size.

        let mut amount_read = 0;
        let blocks = inode.blocks();

        for extent_index in 0..inode.num_extents() as usize {
            let extent = blocks[extent_index];

            for i in extent.start..=extent.end {
                let addr = self.super_block.data_start_address() + i * self.block_size;
                let mut content = unwrap_return_error_voxfs_convertible!(self
                    .handler
                    .read_bytes(addr, self.block_size));

                if self.block_size + (result_bytes.len() as u64) > num_bytes {
                    let end_point = num_bytes - result_bytes.len() as u64;
                    result_bytes.extend_from_slice(&content[..end_point as usize]);
                    amount_read += end_point;
                } else {
                    result_bytes.append(&mut content);
                    amount_read += self.block_size;
                }
            }
        }

        // Now we need to read from any indirect

        let mut next = inode.indirect_pointer();

        while amount_read < num_bytes {
            if next.is_none() {
                return Err(VoxFSError::ExpectedIndirectNode);
            }

            let bytes = unwrap_return_error_voxfs_convertible!(self
                .handler
                .read_bytes(next.unwrap(), self.block_size));
            let indirect = match IndirectINode::from_bytes(&bytes) {
                Some(i) => i,
                None => return Err(VoxFSError::CorruptedIndirectINode),
            };

            for extent in &indirect.extents() {
                for i in extent.start..=extent.end {
                    let addr = self.super_block.data_start_address() + i * self.block_size;
                    let mut content = unwrap_return_error_voxfs_convertible!(self
                        .handler
                        .read_bytes(addr, self.block_size));

                    if self.block_size + (result_bytes.len() as u64) > num_bytes {
                        let end_point = num_bytes - result_bytes.len() as u64;
                        result_bytes.extend_from_slice(&content[..end_point as usize]);
                        amount_read += end_point;
                    } else {
                        result_bytes.append(&mut content);
                        amount_read += self.block_size;
                    }
                }
            }

            next = indirect.next();
        }

        return Ok(result_bytes);
    }

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

        if inode_local_index.is_none() {
            return Err(VoxFSError::CouldNotFindINode);
        }

        let inode_local_index = inode_local_index.unwrap();
        let inode = self.inodes[inode_local_index];

        // Find the last block and how much space of that block is available.
        let mut last_block_extent = inode.blocks()[(inode.num_extents() - 1) as usize];
        let mut next = inode.indirect_pointer();
        let mut previous = None;

        while next.is_some() {
            let bytes = unwrap_return_error_voxfs_convertible!(self
                .handler
                .read_bytes(next.unwrap(), self.block_size));
            let indirect_inode = match IndirectINode::from_bytes(&bytes) {
                Some(i) => i,
                None => return Err(VoxFSError::CorruptedIndirectINode),
            };

            if indirect_inode.next().is_none() {
                last_block_extent = indirect_inode.last_extent().unwrap();
            }

            previous = next;
            next = indirect_inode.next();
        }

        // Check how much space is left in that last block
        let amount_available = self.block_size - (inode.file_size() % self.block_size);

        if amount_available > bytes.len() as u64 {
            // If we can fit all the required data into the space thats available just do that.
            let block_address = self.data_index_to_address(last_block_extent.end);
            let address = block_address + (self.block_size - amount_available);

            unwrap_return_error_voxfs_convertible!(self.handler.write_bytes(&bytes, address));

            self.inodes[inode_local_index].increase_file_size(bytes.len() as u64);
            unwrap_return_error_voxfs_convertible!(self.handler.write_bytes(
                &self.inodes[inode_local_index].to_bytes().to_vec(),
                self.inode_index_to_address(self.inodes[inode_local_index].index())
            ));
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
                let bytes = unwrap_return_error_voxfs_convertible!(self
                    .handler
                    .read_bytes(next.unwrap(), self.block_size));

                let mut indirect_inode = match IndirectINode::from_bytes(&bytes) {
                    Some(i) => i,
                    None => return Err(VoxFSError::CorruptedIndirectINode),
                };

                indirect_inode.set_maximum_extents_blocksize(self.block_size);

                let mut changed_indirect = false;

                while remaining > 0
                    && indirect_inode.append_extent(extents[extents.len() - remaining])
                {
                    remaining -= 1;
                    changed_indirect = true;
                }

                if changed_indirect {
                    unwrap_return_error_voxfs_convertible!(self
                        .handler
                        .write_bytes(&indirect_inode.to_bytes(), next.unwrap()));
                }

                next = indirect_inode.next();
            }

            if remaining > 0 {
                let new_indirect = IndirectINode::new(
                    extents[extents.len() - remaining..].to_vec(),
                    0,
                    self.block_size,
                );

                let index = match self.find_block() {
                    Some(i) => i,
                    None => return Err(VoxFSError::NotEnoughFreeDataBlocks),
                };

                let indirect_address =
                    self.super_block.data_start_address() + index * self.block_size;

                if self.block_bitmap.bit_at(index as usize).unwrap() {
                    return Err(VoxFSError::BlockAlreadyAllocated);
                }

                if !self.block_bitmap.set_bit(index as usize, true) {
                    return Err(VoxFSError::FailedToSetBitmapBit);
                }

                // We need to set a pointer to this new indirect block
                match previous {
                    Some(a) => {
                        let previous_bytes = unwrap_return_error_voxfs_convertible!(self
                            .handler
                            .read_bytes(a, self.block_size));
                        let mut previous_indirect = match IndirectINode::from_bytes(&previous_bytes)
                        {
                            Some(i) => i,
                            None => return Err(VoxFSError::CorruptedIndirectINode),
                        };

                        previous_indirect.set_next(indirect_address);
                        unwrap_return_error_voxfs_convertible!(self
                            .handler
                            .write_bytes(&previous_indirect.to_bytes(), a));
                    }
                    None => {
                        self.inodes[inode_local_index].set_indirect_pointer(Some(indirect_address));
                    }
                }

                // Write the new indirect block
                unwrap_return_error_voxfs_convertible!(self
                    .handler
                    .write_bytes(&new_indirect.to_bytes(), indirect_address));
            }

            // Write as many bytes to the last block as possible
            unwrap_return_error_voxfs_convertible!(self
                .handler
                .write_bytes(&bytes[..amount_available as usize].to_vec(), address));

            // Continue on and write to each of the new extents
            let mut offset = 0;
            for extent in extents {
                for index in extent.start..=extent.end {
                    let index_address = self.data_index_to_address(index);
                    let bytes_end_index = amount_available + ((offset + 1) * self.block_size); // Add 1 to the offset to account for the fact we want to write block_size

                    if bytes_end_index >= bytes.len() as u64 {
                        unwrap_return_error_voxfs_convertible!(self.handler.write_bytes(
                            &bytes[amount_available as usize..].to_vec(),
                            index_address
                        ));
                    } else {
                        unwrap_return_error_voxfs_convertible!(self.handler.write_bytes(
                            &bytes[amount_available as usize..bytes_end_index as usize].to_vec(),
                            index_address
                        ));
                    }

                    offset += 1;
                }
            }

            self.inodes[inode_local_index].increase_file_size(bytes.len() as u64);
            unwrap_return_error_voxfs_convertible!(self.handler.write_bytes(
                &self.inodes[inode_local_index].to_bytes().to_vec(),
                self.inode_index_to_address(self.inodes[inode_local_index].index())
            ));
        }

        return Ok(());
    }

    /// Writes the block availability bit maps
    fn write_bitmaps(&mut self) -> Result<(), VoxFSError<E>> {
        // Write the tags bitmap
        unwrap_return_error_voxfs_convertible!(self
            .handler
            .write_bytes(&self.tag_bitmap.as_bytes(), self.block_size)); // We start at blocksize because of the superblock

        // Write the inodes bitmap
        unwrap_return_error_voxfs_convertible!(self.handler.write_bytes(
            &self.inode_bitmap.as_bytes(),
            self.block_size + self.blocks_for_tag_map * self.block_size
        )); // We start at blocksize because of the superblock then skip the tag map

        // Write the data bitmap
        unwrap_return_error_voxfs_convertible!(self.handler.write_bytes(
            &self.block_bitmap.as_bytes(),
            self.block_size
                + (self.blocks_for_tag_map + self.blocks_for_inode_map) * self.block_size
        )); // We start at blocksize because of the superblock then skip the tag map and inode map

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
        self.write_to_address(self.tag_index_to_address(index as u64),  &tag.to_bytes().to_vec())?;

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

        for i in 0..self.tag_bitmap.len() {
            if self.tag_bitmap.bit_at(i).unwrap() {
                let location = self.tag_index_to_address(i as u64);
                let bytes = self.read_from_address(location, TagBlock::size())?;

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

        for (index, bit) in self.inode_bitmap.flatten_bool().iter().enumerate() {
            // If this bit is marked as taken then read the inode at that location
            if *bit {
                // Read the inode and add it to the memory map
                let address = self.inode_index_to_address(index as u64);
                let bytes = self.read_from_address(address, INode::size())?;

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
        unwrap_return_error_voxfs_convertible!(self.handler.write_bytes(content, address));
        return Ok(());
    }

    /// Read data from an address on the disk.
    #[inline]
    fn read_from_address(
        &self,
        address: u64,
        number_of_bytes: u64,
    ) -> Result<Vec<u8>, VoxFSError<E>> {
        return Ok(unwrap_return_error_voxfs_convertible!(self
            .handler
            .read_bytes(address, number_of_bytes)));
    }
}
