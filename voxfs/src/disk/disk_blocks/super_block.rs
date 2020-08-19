use super::{INode, TagBlock};
use crate::{ByteSerializable, Checksum};
use byteorder::{ByteOrder, LittleEndian};

const CURRENT_VERSION: u8 = 0x00;
const MAGIC: u32 = 0xa1df5000;
const BYTES_PER_INODE: u64 = 2048;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SuperBlock {
    /// Magic used to identify the filesystem
    magic: u32, // the magic is the form 0xa1df5000 & (version number 0x00->0xff)

    /// The size of the blocks
    block_size: u64,
    /// The number of tags. Rounded up to the nearest block alignment.
    tag_count: u64,
    /// The number of inodes. Rounded up to the nearest block alignment.
    inode_count: u64,
    /// The number of data blocks.
    block_count: u64,

    /// The address at which the tags are contained.
    pub tag_start_address: u64,
    /// The address at which the inodes are contained.
    pub inode_start_address: u64,
    /// The address at which data blocks reside.
    pub data_start_address: u64,

    checksum: u8,
    reserved: [u8; 3],
}

impl SuperBlock {
    // Disk size should be all the available disk space for writing data. This should factor exclude the size of the superblock, maps and padding.
    // This method does not set values for addresses.
    pub fn new(block_size: u64, disk_size: u64) -> Self {
        let total_block_count = disk_size / block_size;
        let total_inodes = (total_block_count * block_size) / BYTES_PER_INODE;

        let mut inodes = (total_inodes / 4) * 3;
        let mut tags = total_inodes / 4;

        inodes += (block_size - ((inodes * INode::size()) % block_size)) / INode::size(); // Add enough inodes to fill the blocks.
        tags += (block_size - ((tags * TagBlock::size()) % block_size)) / TagBlock::size(); // Add enough tags to fill the blocks.

        let available_data_blocks = total_block_count
            - ((inodes * INode::size()) / block_size)
            - ((tags * TagBlock::size()) / block_size);

        let mut new = Self {
            magic: MAGIC | (CURRENT_VERSION as u32),
            block_size,
            block_count: available_data_blocks,
            inode_count: inodes,
            tag_count: tags,
            tag_start_address: 0,
            inode_start_address: 0,
            data_start_address: 0,
            checksum: 0,
            reserved: [0u8; 3],
        };

        new.set_checksum();

        return new;
    }

    pub fn blocks_for_inodes(&self) -> u64 {
        return (self.inode_count * INode::size()) / self.block_size;
    }

    pub fn blocks_for_tags(&self) -> u64 {
        return (self.tag_count * TagBlock::size()) / self.block_size;
    }

    /// The number of tags. Rounded up to the nearest block alignment.
    pub fn tag_count(&self) -> u64 {
        return self.tag_count;
    }

    /// The number of inodes. Rounded up to the nearest block alignment.
    pub fn inode_count(&self) -> u64 {
        return self.inode_count;
    }

    /// The number of data blocks.
    pub fn block_count(&self) -> u64 {
        return self.block_count;
    }

    // Dead code since nothing should require this but for consistency it is provided.
    #[allow(dead_code)]
    /// The size of the superblock.
    pub fn size() -> u64 {
        return 64; // 64 bytes
    }
}

impl ByteSerializable for SuperBlock {
    type BytesArrayType = [u8; 64];

    fn to_bytes(&self) -> Self::BytesArrayType {
        let mut bytes = [0u8; 64];
        let mut offset = 0;

        LittleEndian::write_u32(&mut bytes[offset..], self.magic);
        offset += 4;

        LittleEndian::write_u64(&mut bytes[offset..], self.block_size);
        offset += 8;

        LittleEndian::write_u64(&mut bytes[offset..], self.tag_count);
        offset += 8;
        LittleEndian::write_u64(&mut bytes[offset..], self.inode_count);
        offset += 8;
        LittleEndian::write_u64(&mut bytes[offset..], self.block_count);
        offset += 8;

        LittleEndian::write_u64(&mut bytes[offset..], self.tag_start_address);
        offset += 8;
        LittleEndian::write_u64(&mut bytes[offset..], self.inode_start_address);
        offset += 8;
        LittleEndian::write_u64(&mut bytes[offset..], self.data_start_address);
        offset += 8;

        bytes[offset] = self.checksum;
        //offset += 1; // Increment if in further revisions data is added beyond this point

        // bytes 62, 63, 64 are reserved

        return bytes;
    }

    fn from_bytes(bytes: &[u8]) -> Option<Self>
    where
        Self: core::marker::Sized,
    {
        if bytes.len() < 40 {
            return None;
        }

        let mut offset = 0;

        let magic: u32;

        let block_size: u64;

        let tag_count: u64;
        let inode_count: u64;
        let block_count: u64;

        let tag_start_address: u64;
        let inode_start_address: u64;
        let data_start_address: u64;

        let checksum: u8;

        magic = LittleEndian::read_u32(&bytes[offset..]);
        offset += 4;

        block_size = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;

        tag_count = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;
        inode_count = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;
        block_count = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;

        tag_start_address = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;
        inode_start_address = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;
        data_start_address = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;

        checksum = bytes[offset];
        //offset += 1;  // Increment if in further revisions data is added beyond this point

        let res = Self {
            magic,
            block_size,
            tag_count,
            inode_count,
            block_count,
            tag_start_address,
            inode_start_address,
            data_start_address,
            checksum,
            reserved: [0u8; 3],
        };

        if res.perform_checksum() {
            return Some(res);
        } else {
            return None;
        }
    }

    fn generic_bytes_rep(bytes: &Self::BytesArrayType) -> &[u8] {
        return bytes;
    }
}

impl Checksum for SuperBlock {
    fn set_checksum(&mut self) {
        self.checksum = self.calculate_checksum();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const DEFAULT_BLOCK_SIZE: u64 = 4096; // Just for testing

    #[test]
    fn create_new() {
        let disk_size = 4096 * 250;
        let block_size = DEFAULT_BLOCK_SIZE as u64;

        let block = SuperBlock::new(block_size, disk_size);

        assert_eq!(
            block,
            SuperBlock {
                magic: MAGIC | (CURRENT_VERSION as u32),
                block_size,
                tag_count: 128,
                inode_count: 384,
                block_count: 218,
                tag_start_address: 0,
                inode_start_address: 0,
                data_start_address: 0,
                checksum: 69,
                reserved: [0u8; 3]
            }
        );

        assert!(block.perform_checksum());
    }

    #[test]
    fn test_to_bytes() {
        let disk_size = 4096 * 250;
        let block_size = DEFAULT_BLOCK_SIZE as u64;

        let block = SuperBlock::new(block_size, disk_size);

        let bytes = {
            let mut res = [0u8; 64];

            // Magic
            res[0] = 0x00;
            res[1] = 0x50;
            res[2] = 0xdf;
            res[3] = 0xa1;

            // Block size
            res[4] = 0x00;
            res[5] = 0x10;

            // Tag count
            res[12] = 128;

            // Inode count
            res[20] = 0x80;
            res[21] = 0x01;

            // Block count
            res[28] = 218;

            res[60] = 69;

            res
        };

        assert_eq!(block.to_bytes().to_vec(), bytes.to_vec());
    }

    #[test]
    fn test_from_bytes() {
        let disk_size = 4096 * 250;
        let block_size = DEFAULT_BLOCK_SIZE as u64;

        let block = SuperBlock::new(block_size, disk_size);

        let bytes = {
            let mut res = [0u8; 64];

            // Magic
            res[0] = 0x00;
            res[1] = 0x50;
            res[2] = 0xdf;
            res[3] = 0xa1;

            // Block size
            res[4] = 0x00;
            res[5] = 0x10;

            // Tag count
            res[12] = 128;

            // Inode count
            res[20] = 0x80;
            res[21] = 0x01;

            // Block count
            res[28] = 218;

            res[60] = 69;

            res
        };

        assert_eq!(SuperBlock::from_bytes(&bytes).unwrap(), block);
    }
}
