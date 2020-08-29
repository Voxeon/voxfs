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
// TODO: 4. Support appending to files.
// TODO: 5. Support overwriting files.

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

macro_rules! unwrap_error_aidfs_convertible {
    ($v:expr) => {
        match $v {
            Ok(val) => val,
            Err(e) => return Err(e.into_voxfs_error()),
        }
    };
}

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
        let disk_size = unwrap_error_aidfs_convertible!(handler.disk_size());
        let block_size = DEFAULT_BLOCK_SIZE;

        if block_size % 64 != 0 {
            return Err(VoxFSError::InvalidBlockSize);
        }

        let mut super_block = SuperBlock::new(block_size, disk_size);
        unwrap_error_aidfs_convertible!(handler.zero_range(0, block_size)); // Zero the first block.

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

        let blocks_for_tag_map = bitmap_rounded_to_alignment!(tag_bitmap, block_size);
        let blocks_for_inode_map = bitmap_rounded_to_alignment!(inode_bitmap, block_size);
        let blocks_for_block_map = bitmap_rounded_to_alignment!(block_bitmap, block_size);

        offset += (blocks_for_tag_map + blocks_for_inode_map + blocks_for_block_map) * block_size;

        // Setup the addresses
        super_block.set_tag_start_address(offset);
        offset += block_size * super_block.blocks_for_tags();

        super_block.set_inode_start_address(offset);
        offset += block_size * super_block.blocks_for_inodes();

        super_block.set_data_start_address(offset);

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

        let block_size = DEFAULT_BLOCK_SIZE;
        let first_block = unwrap_error_aidfs_convertible!(handler.read_bytes(0, block_size));
        let super_block = match SuperBlock::from_bytes(&first_block) {
            Some(b) => b,
            None => return Err(VoxFSError::CorruptedSuperBlock),
        };

        let block_size = super_block.block_size();
        let blocks_for_tag_map = rounded_to_alignment!(super_block.tag_count(), block_size);
        let blocks_for_inode_map = rounded_to_alignment!(super_block.inode_count(), block_size);
        let blocks_for_block_map = rounded_to_alignment!(super_block.block_count(), block_size);

        let tag_bitmaps_bytes = unwrap_error_aidfs_convertible!(
            handler.read_bytes(block_size, blocks_for_tag_map * block_size)
        );
        let inode_bitmaps_bytes = unwrap_error_aidfs_convertible!(handler.read_bytes(
            block_size + blocks_for_tag_map * block_size,
            blocks_for_inode_map * block_size
        ));
        let data_bitmaps_bytes = unwrap_error_aidfs_convertible!(handler.read_bytes(
            block_size + (blocks_for_tag_map + blocks_for_inode_map) * block_size,
            blocks_for_block_map * block_size
        ));

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
        self.tags.push(tag);

        return Ok(tag);
    }

    /// Deletes a tag
    pub fn delete_tag(&mut self, index: u64) -> Result<(), VoxFSError<E>> {
        let mut local_index = None;
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

        let local_index = local_index.unwrap();
        let local_tag = local_tag.unwrap();

        let mut data_block_indices: Vec<u64> = Vec::new(); // index of data block
        let mut current_indirect = local_tag.indirect_pointer();

        while current_indirect.is_some() {
            let address = current_indirect.unwrap();
            let index = self.address_to_data_index(address);
            let bytes =
                unwrap_error_aidfs_convertible!(self.handler.read_bytes(address, self.block_size));
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

        if !self.tag_bitmap.set_bit(local_tag.index() as usize, false) {
            return Err(VoxFSError::FailedToFreeTag);
        }

        self.tags.remove(local_index);

        self.write_bitmaps()?;

        return Ok(());
    }

    /// List the tags stored on the disk.
    pub fn list_tags(&self) -> Vec<TagBlock> {
        return self.tags.clone();
    }

    /// List the inodes on the disk, this method doesn't reload them.
    pub fn list_inodes(&self) -> Vec<INode> {
        return self.inodes.clone();
    }

    /// Add an inode to a tag
    pub fn apply_tag(&mut self, tag_index: u64, inode: &INode) -> Result<(), VoxFSError<E>> {
        let mut tag_self_index = None;

        for (i, tag) in self.tags.iter().enumerate() {
            if tag.index() == tag_index {
                tag_self_index = Some(i);
                break;
            }
        }

        let tag_self_index = match tag_self_index {
            Some(i) => i,
            None => return Err(VoxFSError::CouldNotFindTag),
        };

        if self.tags[tag_self_index].number_of_pointers() >= TagBlock::MAXIMUM_LOCAL_MEMBERS {
            /*
            Two things must be checked.
            1. Has this tag been applied before to this INode.
            2. Where is a free spot?
             */

            for member in &self.tags[tag_self_index].members() {
                if *member == inode.index() {
                    return Err(VoxFSError::TagAlreadyAppliedToINode);
                }
            }

            let mut next_address = self.tags[tag_self_index].indirect_pointer();
            let mut previous_indirect = None;
            let mut previous_indirect_location = None;
            let mut free_indirect_tag_address: Option<u64> = None; // This tracks the location of an available spot in an inode

            while next_address.is_some() {
                let bytes = unwrap_error_aidfs_convertible!(self
                    .handler
                    .read_bytes(next_address.unwrap(), self.block_size));

                let indirect_tag = match IndirectTagBlock::from_bytes(&bytes) {
                    Some(t) => t,
                    None => return Err(VoxFSError::CorruptedIndirectTag),
                };

                let members = indirect_tag.members();

                for i in 0..indirect_tag.number_of_members() as usize {
                    let member = members[i];
                    if member == inode.index() {
                        return Err(VoxFSError::TagAlreadyAppliedToINode);
                    }
                }

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

            if free_indirect_tag_address.is_none() {
                // Create a new indirect tag
                let indirect_tag = IndirectTagBlock::new(
                    self.tags[tag_self_index].index(),
                    vec![inode.index()],
                    0,
                    self.block_size,
                );

                let index = match self.find_block() {
                    Some(index) => index,
                    None => return Err(VoxFSError::NotEnoughFreeDataBlocks),
                };

                let location = self.super_block.data_start_address() + index * self.block_size;

                self.block_bitmap.set_bit(index as usize, true);
                unwrap_error_aidfs_convertible!(self.handler.write_bytes(
                    &indirect_tag.to_bytes_padded(self.block_size as usize),
                    location
                ));
                self.write_bitmaps()?;

                match previous_indirect {
                    Some(mut i) => {
                        i.set_next(location);

                        unwrap_error_aidfs_convertible!(self.handler.write_bytes(
                            &i.to_bytes_padded(self.block_size as usize),
                            previous_indirect_location.unwrap()
                        ));
                    }
                    None => {
                        self.tags[tag_self_index].set_indirect(location);
                        unwrap_error_aidfs_convertible!(self.handler.write_bytes(
                            &self.tags[tag_self_index].to_bytes().to_vec(),
                            self.super_block.tag_start_address()
                                + self.tags[tag_self_index].index() * TagBlock::size()
                        ));
                    }
                }
            } else {
                let mut indirect =
                    match IndirectTagBlock::from_bytes(&unwrap_error_aidfs_convertible!(self
                        .handler
                        .read_bytes(free_indirect_tag_address.unwrap(), self.block_size)))
                    {
                        Some(i) => i,
                        None => return Err(VoxFSError::CorruptedIndirectTag),
                    };

                indirect.set_block_size(self.block_size);

                if !indirect.append_member(inode.index()) {
                    return Err(VoxFSError::FailedIndirectTagAppend);
                }

                unwrap_error_aidfs_convertible!(self.handler.write_bytes(
                    &indirect.to_bytes_padded(self.block_size as usize),
                    free_indirect_tag_address.unwrap()
                ));
            }
        } else {
            // Check if this tag has already been applied.
            for i in 0..self.tags[tag_self_index].number_of_pointers() {
                if self.tags[tag_self_index].member_at(i) == inode.index() {
                    return Err(VoxFSError::TagAlreadyAppliedToINode);
                }
            }

            self.tags[tag_self_index].append_member(inode.index());

            unwrap_error_aidfs_convertible!(self.handler.write_bytes(
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
                let bytes = unwrap_error_aidfs_convertible!(self
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
                                unwrap_error_aidfs_convertible!(self.handler.write_bytes(
                                    &cp.to_bytes_padded(self.block_size as usize),
                                    parent_address.unwrap()
                                ));
                            }
                            None => {
                                self.tags[tag_local_index].set_indirect_optional(block.next());
                                self.block_bitmap.set_bit(index as usize, false);

                                self.write_bitmaps()?;
                                unwrap_error_aidfs_convertible!(self.handler.write_bytes(
                                    &self.tags[tag_local_index].to_bytes().to_vec(),
                                    self.super_block.tag_start_address()
                                        + self.tags[tag_local_index].index() * TagBlock::size()
                                ));

                                //unwrap_error_aidfs_convertible!(self.handler.write_bytes(&tag.to_bytes().to_vec(), self.super_block.tag_start_address() + tag.index() * TagBlock::size()));
                            }
                        }
                    } else {
                        unwrap_error_aidfs_convertible!(self.handler.write_bytes(
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
                    let bytes = unwrap_error_aidfs_convertible!(self
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
    pub fn create_new_file_first_free(
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

        for (start, end) in &blocks {
            for i in *start..=*end {
                if self.block_bitmap.bit_at(i as usize).unwrap() {
                    return Err(VoxFSError::BlockAlreadyAllocated);
                } else {
                    self.block_bitmap.set_bit(i as usize, true);

                    if contents_offset + self.block_size > contents.len() as u64 {
                        unwrap_error_aidfs_convertible!(self.handler.write_bytes(
                            &contents[contents_offset as usize..].to_vec(),
                            self.super_block.data_start_address() + i * self.block_size,
                        ));
                    } else {
                        unwrap_error_aidfs_convertible!(self.handler.write_bytes(
                            &contents[contents_offset as usize
                                ..(contents_offset + self.block_size) as usize]
                                .to_vec(),
                            self.super_block.data_start_address() + i * self.block_size,
                        ));
                    }

                    contents_offset += self.block_size;
                }
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
                unwrap_error_aidfs_convertible!(self
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

        unwrap_error_aidfs_convertible!(self.handler.write_bytes(
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

    /// Reads an entire file from the disk
    pub fn read_file(&self, inode: &INode) -> Result<Vec<u8>, VoxFSError<E>> {
        return self.read_file_bytes(inode, inode.file_size());
    }

    /// Reads a specified amount of bytes from the start of a file. If num_bytes is greater than the length of the file only up to the size of the file will be returned.
    pub fn read_file_bytes(&self, inode: &INode, num_bytes: u64) -> Result<Vec<u8>, VoxFSError<E>> {
        let mut result_bytes = Vec::new();

        let num_bytes = {
            if num_bytes > inode.file_size() {
                inode.file_size()
            } else {
                num_bytes
            }
        };

        // Whilst it is possible to read large chunks, for the sake of simplicity for the driver implementor, we will not read amounts larger than the block size.

        for extent in &inode.blocks() {
            for i in extent.start..=extent.end {
                let addr = self.super_block.data_start_address() + i * self.block_size;
                let mut content =
                    unwrap_error_aidfs_convertible!(self.handler.read_bytes(addr, self.block_size));

                if self.block_size + (result_bytes.len() as u64) > num_bytes {
                    let end_point = num_bytes - result_bytes.len() as u64;
                    result_bytes.extend_from_slice(&content[..end_point as usize]);
                } else {
                    result_bytes.append(&mut content);
                }
            }
        }

        return Ok(result_bytes);
    }

    /// Writes the block availability bit maps
    fn write_bitmaps(&mut self) -> Result<(), VoxFSError<E>> {
        // Write the tags bitmap
        unwrap_error_aidfs_convertible!(self
            .handler
            .write_bytes(&self.tag_bitmap.as_bytes(), self.block_size)); // We start at blocksize because of the superblock

        // Write the inodes bitmap
        unwrap_error_aidfs_convertible!(self.handler.write_bytes(
            &self.inode_bitmap.as_bytes(),
            self.block_size + self.blocks_for_tag_map * self.block_size
        )); // We start at blocksize because of the superblock then skip the tag map

        // Write the data bitmap
        unwrap_error_aidfs_convertible!(self.handler.write_bytes(
            &self.block_bitmap.as_bytes(),
            self.block_size
                + (self.blocks_for_tag_map + self.blocks_for_inode_map) * self.block_size
        )); // We start at blocksize because of the superblock then skip the tag map and inode map

        return Ok(());
    }

    /// Stores a new tag in the first available spot. It will set the index field of the tag block
    fn store_tag_first_free(&mut self, mut tag: TagBlock) -> Result<TagBlock, VoxFSError<E>> {
        let index = match self
            .tag_bitmap
            .find_next_0_index_up_to(self.super_block.tag_count() as usize)
        {
            Some(index) => index,
            None => return Err(VoxFSError::NoFreeInode),
        };

        tag.set_index(index as u64);

        unwrap_error_aidfs_convertible!(self.handler.write_bytes(
            &tag.to_bytes().to_vec(),
            self.super_block.tag_start_address() + (index as u64) * TagBlock::size()
        ));

        if !self.tag_bitmap.set_bit(index, true) {
            panic!("Unexpected fail."); // This should never happen but if it does then its a developer error so panic.
        }

        self.write_bitmaps()?;

        return Ok(tag);
    }

    /// Load a list of the tags from the disk
    fn load_tags(&self) -> Result<Vec<TagBlock>, VoxFSError<E>> {
        let mut tags = Vec::new();

        for i in 0..self.tag_bitmap.len() {
            if self.tag_bitmap.bit_at(i).unwrap() {
                let location = self.super_block.tag_start_address() + (i as u64) * TagBlock::size();

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

    /// Reads and loads all the inodes on the filesystem.
    fn load_inodes(&mut self) -> Result<Vec<INode>, VoxFSError<E>> {
        let mut inodes: Vec<INode> = Vec::new();

        for (index, bit) in self.inode_bitmap.flatten_bool().iter().enumerate() {
            if *bit {
                let address =
                    self.super_block.inode_start_address() + (index as u64) * INode::size();

                let bytes = unwrap_error_aidfs_convertible!(self
                    .handler
                    .read_bytes(address, INode::size()));
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
}
