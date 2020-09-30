use crate::ByteSerializable;
use crate::Checksum;
use alloc::string::String;
use alloc::{vec, vec::Vec};
use byteorder::{ByteOrder, LittleEndian};
use chrono::{DateTime, TimeZone, Utc};

const MAX_INODE_NAME_LENGTH: usize = 125;
const INODE_EXTENT_COUNT: usize = 5;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(packed)]
/// The possible flags for a file.
pub struct INodeFlags {
    /// valid, a bit that if set indicates that this points to a valid file. Otherwise it is safe to override.
    valid: bool,
    read: bool,
    write: bool,
    execute: bool,
    // reserved: [bool; 4],  There are 4 reserved bits for future use
}

#[derive(Copy, Clone)]
/// A Node used to indicate where a file's metadata. It is of length 256 bytes
pub struct INode {
    /// Index, a unique number representing this inode's location in the inode map
    index: u64,
    /// name, 125 bytes constant filled with null bytes otherwise
    name: [char; MAX_INODE_NAME_LENGTH],
    /// size in bytes, this is the actual size NOT the on disk size.
    size: u64,
    /// flags (v,r,w,e), bits 5 - 8 are reserved
    flags: INodeFlags,
    /// access time, nano seconds since unix epoch
    access_time: u64,
    /// modified time, nano seconds since unix epoch
    modified_time: u64,
    /// creation time, nano seconds since unix epoch
    creation_time: u64,
    /// checksum, sum of the bytes with wrapping addition must be zero.
    checksum: u8,
    /// indirect block points to a block that only contains a list of pointers to other blocks. It IS an address not an index.
    indirect_block: u64,
    /// The number of extents stored in THIS inode excluding indirect inodes.
    num_extents: u8,
    /// pointers to blocks, if a space is unused it will be represented simply by 0
    blocks: [Extent; INODE_EXTENT_COUNT],
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
/// An extent represents a range of blocks, it is inclusive at BOTH ends. It uses data block indexes.
pub struct Extent {
    pub start: u64,
    pub end: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// This takes a whole block.
pub struct IndirectINode {
    /// Checksum
    checksum: u8,
    /// Reserved byte
    reserved: u8,
    /// Pointer to another IndirectINode, an address.
    next: u64,
    /// The number of extents stored in THIS inode excluding indirect inodes.
    num_extents: u16,
    /// As many extents as  we can represent, a maximum of 65,355 entries.
    pointers: Vec<Extent>,

    /// This is NOT serialized or placed on disk, it is for the methods to know the limit for the number of extents.
    pub maximum_extents: u64,
}

impl INodeFlags {
    pub fn new(valid: bool, read: bool, write: bool, execute: bool) -> Self {
        return Self {
            valid,
            read,
            write,
            execute,
        };
    }

    pub fn from_u8(n: u8) -> Self {
        let valid = ((n >> 7) & 1) == 1;
        let read = ((n >> 6) & 1) == 1;
        let write = ((n >> 5) & 1) == 1;
        let execute = ((n >> 4) & 1) == 1;

        return Self::new(valid, read, write, execute);
    }

    pub fn to_u8(&self) -> u8 {
        let mut res = 0b0000_0000;

        if self.valid {
            res |= 1 << 7;
        }

        if self.read {
            res |= 1 << 6;
        }

        if self.write {
            res |= 1 << 5;
        }

        if self.execute {
            res |= 1 << 4;
        }

        return res;
    }
}

impl INode {
    pub fn new(
        index: u64,
        str_name: &str,
        size: u64,
        flags: INodeFlags,
        access_time: DateTime<Utc>,
        modified_time: DateTime<Utc>,
        creation_time: DateTime<Utc>,
        indirect_pointer: u64,
        num_extents: u8,
        blocks: [Extent; INODE_EXTENT_COUNT],
    ) -> Self {
        let mut name: [char; MAX_INODE_NAME_LENGTH] = ['\0'; MAX_INODE_NAME_LENGTH];

        for (i, c) in str_name.chars().enumerate() {
            if i >= MAX_INODE_NAME_LENGTH {
                break;
            }

            name[i] = c;
        }

        let mut res = Self {
            index,
            name,
            size,
            flags,
            access_time: access_time.timestamp_nanos() as u64,
            modified_time: modified_time.timestamp_nanos() as u64,
            creation_time: creation_time.timestamp_nanos() as u64,
            checksum: 0,
            indirect_block: indirect_pointer,
            num_extents,
            blocks,
        };

        res.set_checksum();

        return res;
    }

    pub(crate) const fn size() -> u64 {
        return 256;
    }

    pub fn name(&self) -> String {
        let mut first_null_byte = self.name.len();
        for (i, ch) in self.name.iter().enumerate() {
            if *ch == '\0' {
                first_null_byte = i;
                break;
            }
        }

        return self.name[..first_null_byte].iter().collect();
    }

    pub(crate) fn indirect_pointer(&self) -> Option<u64> {
        if self.indirect_block == 0 {
            return None;
        } else {
            return Some(self.indirect_block);
        }
    }

    pub(crate) fn set_indirect_pointer(&mut self, new: Option<u64>) {
        match new {
            Some(n) => self.indirect_block = n,
            None => self.indirect_block = 0,
        }

        self.set_checksum();
    }

    pub fn index(&self) -> u64 {
        return self.index;
    }

    pub fn file_size(&self) -> u64 {
        return self.size;
    }

    pub fn access_time(&self) -> DateTime<Utc> {
        return Utc.timestamp_nanos(self.access_time as i64);
    }

    pub fn modified_time(&self) -> DateTime<Utc> {
        return Utc.timestamp_nanos(self.modified_time as i64);
    }

    pub fn creation_time(&self) -> DateTime<Utc> {
        return Utc.timestamp_nanos(self.creation_time as i64);
    }

    pub(crate) fn increase_file_size(&mut self, amount: u64) {
        self.size += amount;
        self.set_checksum();
    }

    pub(crate) fn append_extent(&mut self, extent: Extent) -> bool {
        if self.num_extents >= INODE_EXTENT_COUNT as u8 {
            return false;
        }

        self.blocks[self.num_extents as usize] = extent;
        self.num_extents += 1;

        self.set_checksum();

        return true;
    }

    #[inline]
    pub(crate) fn blocks(&self) -> [Extent; INODE_EXTENT_COUNT] {
        return self.blocks;
    }

    #[inline]
    pub(crate) fn num_extents(&self) -> u8 {
        return self.num_extents;
    }

    #[inline]
    pub(crate) const fn max_extents() -> u8 {
        return INODE_EXTENT_COUNT as u8;
    }
}

impl Checksum for INode {
    fn set_checksum(&mut self) {
        self.checksum = 0; // For the purpose of calculation
        self.checksum = self.calculate_checksum();
    }
}

impl ByteSerializable for INode {
    type BytesArrayType = [u8; 256];

    fn to_bytes(&self) -> Self::BytesArrayType {
        let mut bytes = vec![0u8; 256];
        let mut offset = 0;

        LittleEndian::write_u64(&mut bytes, self.index);

        offset += 8;

        for (i, c) in self.name.iter().enumerate() {
            bytes[offset + i] = *c as u8;
        }

        offset += MAX_INODE_NAME_LENGTH;

        LittleEndian::write_u64(&mut bytes[offset..], self.size);
        offset += 8;

        bytes[offset] = self.flags.to_u8();
        offset += 1;

        LittleEndian::write_u64(&mut bytes[offset..], self.access_time);
        offset += 8;
        LittleEndian::write_u64(&mut bytes[offset..], self.modified_time);
        offset += 8;
        LittleEndian::write_u64(&mut bytes[offset..], self.creation_time);
        offset += 8;
        bytes[offset] = self.checksum;
        offset += 1;
        LittleEndian::write_u64(&mut bytes[offset..], self.indirect_block);
        offset += 8;

        bytes[offset] = self.num_extents;
        offset += 1;

        for block in self.blocks.iter() {
            LittleEndian::write_u64(&mut bytes[offset..], block.start);
            offset += 8;
            LittleEndian::write_u64(&mut bytes[offset..], block.end);
            offset += 8;
        }

        let mut res = [0u8; 256];
        res.copy_from_slice(&bytes);
        return res;
    }

    /// Performs the checksum check.
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 256 {
            return None;
        }

        let mut offset = 0;

        let index: u64;
        let mut name = ['\0'; MAX_INODE_NAME_LENGTH];
        let size: u64;
        let flags: INodeFlags;
        let access_time: u64;
        let modified_time: u64;
        let creation_time: u64;
        let checksum: u8;
        let indirect_block: u64;
        let num_extents: u8;
        let mut blocks = [Extent::zeroed(); INODE_EXTENT_COUNT];

        index = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;

        let mut i = 0;
        for ch in &bytes[offset..offset + MAX_INODE_NAME_LENGTH] {
            name[i] = *ch as char;
            i += 1;
        }

        offset += MAX_INODE_NAME_LENGTH;

        size = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;

        flags = INodeFlags::from_u8(bytes[offset]);
        offset += 1;

        access_time = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;

        modified_time = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;

        creation_time = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;

        checksum = bytes[offset];
        offset += 1;

        indirect_block = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;

        num_extents = bytes[offset];
        offset += 1;

        let mut i = 0;
        for _ in (offset..256).step_by(16) {
            blocks[i].start = LittleEndian::read_u64(&bytes[offset..]);
            offset += 8;
            blocks[i].end = LittleEndian::read_u64(&bytes[offset..]);
            offset += 8;

            i += 1;
        }

        let s = Self {
            index,
            name,
            size,
            flags,
            access_time,
            modified_time,
            creation_time,
            checksum,
            indirect_block,
            num_extents,
            blocks,
        };

        if s.perform_checksum() {
            return Some(s);
        } else {
            return None;
        }
    }

    fn generic_bytes_rep(bytes: &Self::BytesArrayType) -> &[u8] {
        return bytes;
    }
}

impl core::cmp::PartialEq for INode {
    fn eq(&self, other: &Self) -> bool {
        let mut name_identical = true;

        for i in 0..MAX_INODE_NAME_LENGTH {
            if other.name[i] != self.name[i] {
                name_identical = false;

                break;
            }
        }

        return self.index == other.index
            && name_identical
            && self.size == other.size
            && self.flags == other.flags
            && self.access_time == other.access_time
            && self.modified_time == other.modified_time
            && self.creation_time == other.creation_time
            && self.checksum == other.checksum
            && self.indirect_block == other.indirect_block
            && self.num_extents == other.num_extents
            && self.blocks == other.blocks;
    }
}

impl core::fmt::Debug for INode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let name_str: alloc::string::String = self.name.iter().collect();
        return f
            .debug_struct("INode")
            .field("index", &self.index)
            .field("name", &name_str)
            .field("size", &self.size)
            .field("flags", &self.flags)
            .field("access_time", &self.access_time)
            .field("modified_time", &self.modified_time)
            .field("creation_time", &self.creation_time)
            .field("checksum", &self.checksum)
            .field("indirect_block", &self.indirect_block)
            .field("num_extents", &self.num_extents)
            .field("blocks", &self.blocks)
            .finish();
    }
}

impl Extent {
    #[inline]
    pub fn zeroed() -> Self {
        return Self { start: 0, end: 0 };
    }

    #[inline]
    pub fn size() -> u64 {
        return 16; // start: 8 bytes, end: 8 bytes
    }
}

impl IndirectINode {
    /// The size in bytes of the fixed length elements within an indirect INode.
    const NON_EXPANDABLE_SIZE: u64 = 1 + 1 + 2 + 8;

    /// Constructs a new `IndirectINode`, block_size should be greater than 256 at a minimum.
    pub fn new(pointers: Vec<Extent>, next: u64, block_size: u64) -> Self {
        assert!(block_size > 256);
        assert!(pointers.len() < u16::MAX as usize);
        let maximum_extents = Self::max_extents_for_blocksize(block_size);
        assert!(pointers.len() <= maximum_extents as usize);

        let mut res = Self {
            checksum: 0,
            reserved: 0,
            num_extents: pointers.len() as u16,
            pointers,
            next,
            maximum_extents,
        };

        res.set_checksum();

        return res;
    }

    pub fn next(&self) -> Option<u64> {
        if self.next == 0 {
            return None;
        } else {
            return Some(self.next);
        }
    }

    pub fn set_next(&mut self, next: u64) {
        self.next = next;
        self.set_checksum();
    }

    pub fn next_is_set(&self) -> bool {
        return self.next != 0;
    }

    pub fn append_extent(&mut self, extent: Extent) -> bool {
        if self.num_extents as u64 >= self.maximum_extents {
            return false;
        }

        self.pointers.push(extent);
        self.num_extents += 1;

        self.set_checksum();

        return true;
    }

    pub fn remove_extent(&mut self, index: u16) -> bool {
        if index >= self.num_extents {
            return false;
        }

        self.pointers.remove(index as usize);
        self.num_extents -= 1;

        self.set_checksum();

        return true;
    }

    pub fn last_extent(&self) -> Option<Extent> {
        return match self.pointers.last() {
            Some(p) => Some(*p),
            None => None,
        };
    }

    pub fn extents(&self) -> Vec<Extent> {
        return self.pointers.clone();
    }

    pub fn capacity(&self) -> u64 {
        return self.maximum_extents;
    }

    /// Sets the maximum number of extents for this blocksize. Allows for the use of append.
    pub fn set_maximum_extents_blocksize(&mut self, blocksize: u64) {
        self.maximum_extents = Self::max_extents_for_blocksize(blocksize);
    }

    #[inline]
    pub fn max_extents_for_blocksize(blocksize: u64) -> u64 {
        return (blocksize - Self::NON_EXPANDABLE_SIZE) / Extent::size();
    }
}

impl ByteSerializable for IndirectINode {
    type BytesArrayType = Vec<u8>;

    fn to_bytes(&self) -> Self::BytesArrayType {
        let mut bytes = Vec::new();
        let mut working = [0u8; 8];

        bytes.push(self.checksum);
        bytes.push(self.reserved);

        LittleEndian::write_u64(&mut working, self.next);
        bytes.extend_from_slice(&working);

        LittleEndian::write_u16(&mut working, self.num_extents);
        bytes.extend_from_slice(&working[0..2]);

        for extent in &self.pointers {
            LittleEndian::write_u64(&mut working, extent.start);
            bytes.extend_from_slice(&working);

            LittleEndian::write_u64(&mut working, extent.end);
            bytes.extend_from_slice(&working);
        }

        return bytes;
    }

    // Performs a checksum check
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::NON_EXPANDABLE_SIZE as usize {
            return None;
        }

        let checksum;
        let reserved;
        let next;
        let num_extents;
        let mut extents = Vec::new();

        let mut offset = 0;

        checksum = bytes[offset];
        offset += 1;

        reserved = bytes[offset];
        offset += 1;

        next = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;

        num_extents = LittleEndian::read_u16(&bytes[offset..]);
        offset += 2;

        for _ in 0..num_extents {
            let start = LittleEndian::read_u64(&bytes[offset..]);
            offset += 8;
            let end = LittleEndian::read_u64(&bytes[offset..]);
            offset += 8;

            extents.push(Extent { start, end });
        }

        let res = Self {
            checksum,
            reserved,
            num_extents,
            pointers: extents,
            next,
            maximum_extents: 0,
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

impl Checksum for IndirectINode {
    fn set_checksum(&mut self) {
        self.checksum = 0; // For the purpose of calculation
        self.checksum = self.calculate_checksum();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod flags {
        use super::*;

        #[test]
        fn test_to_u8() {
            let flags = INodeFlags::new(true, false, true, false);
            assert_eq!(flags.to_u8(), 0b1010_0000);
        }

        #[test]
        fn test_from_u8() {
            let flags = INodeFlags::new(true, false, true, false);
            assert_eq!(flags, INodeFlags::from_u8(0b1010_0000));
        }
    }

    mod inode {
        use super::*;

        #[test]
        fn test_new() {
            let mut blocks = [Extent::zeroed(); 5];
            blocks[0].start = 0x123456;
            blocks[0].end = 0x123456;

            let node = INode::new(
                0,
                "new file",
                246,
                INodeFlags::new(true, true, false, false),
                DateTime::from(
                    DateTime::parse_from_rfc2822("Wed, 18 Feb 2015 23:16:09 +0000").unwrap(),
                ),
                DateTime::from(
                    DateTime::parse_from_rfc2822("Thu, 19 Feb 2015 23:16:09 +0000").unwrap(),
                ),
                DateTime::from(
                    DateTime::parse_from_rfc2822("Fri, 20 Feb 2015 23:16:09 +0000").unwrap(),
                ),
                0,
                1,
                blocks,
            );

            assert!(node.perform_checksum());
        }

        #[test]
        fn test_to_bytes() {
            let mut blocks = [Extent::zeroed(); 5];
            blocks[0].end = 0x123456;
            blocks[0].start = 0x2;

            let node = INode::new(
                1,
                "name",
                246,
                INodeFlags::new(true, true, false, false),
                DateTime::from(
                    DateTime::parse_from_rfc2822("Wed, 18 Feb 2015 23:16:09 +0000").unwrap(),
                ),
                DateTime::from(
                    DateTime::parse_from_rfc2822("Wed, 18 Feb 2015 23:16:10 +0000").unwrap(),
                ),
                DateTime::from(
                    DateTime::parse_from_rfc2822("Wed, 18 Feb 2015 23:16:11 +0000").unwrap(),
                ),
                0,
                1,
                blocks,
            );

            assert!(node.perform_checksum());

            let bytes = node.to_bytes();
            let mut comp = [0u8; 256];
            comp[0] = 1; // Index

            comp[8] = b'n'; // Name
            comp[9] = b'a';
            comp[10] = b'm';
            comp[11] = b'e';

            comp[133] = 246; // Size
            comp[141] = 0b1100_0000; // Flags

            // Access Time: 0x13c4228c8058fa00
            comp[142] = 0x00;
            comp[143] = 0xfa;
            comp[144] = 0x58;
            comp[145] = 0x80;
            comp[146] = 0x8c;
            comp[147] = 0x22;
            comp[148] = 0xc4;
            comp[149] = 0x13;

            // Modified Time: 0x13c4228cbbf3c400
            comp[150] = 0x00;
            comp[151] = 0xc4;
            comp[152] = 0xf3;
            comp[153] = 0xbb;
            comp[154] = 0x8c;
            comp[155] = 0x22;
            comp[156] = 0xc4;
            comp[157] = 0x13;

            // Creation Time: 0x13c4228cf78e8e00
            comp[158] = 0x00;
            comp[159] = 0x8e;
            comp[160] = 0x8e;
            comp[161] = 0xf7;
            comp[162] = 0x8c;
            comp[163] = 0x22;
            comp[164] = 0xc4;
            comp[165] = 0x13;

            // Checksum
            comp[166] = 35;

            // indirect pointer
            comp[167] = 0;

            comp[175] = 0x1; // Size

            // blocks
            comp[176] = 0x2;

            comp[184] = 0x56;
            comp[185] = 0x34;
            comp[186] = 0x12;

            assert_eq!(Vec::from(&bytes[..]), Vec::from(&comp[..]));
        }

        #[test]
        fn test_from_bytes() {
            let comp = {
                let mut comp = [0u8; 256];

                comp[0] = 1; // Index

                comp[8] = b'n'; // Name
                comp[9] = b'a';
                comp[10] = b'm';
                comp[11] = b'e';

                comp[133] = 246; // Size
                comp[141] = 0b1100_0000; // Flags

                // Access Time: 0x13c4228c8058fa00
                comp[142] = 0x00;
                comp[143] = 0xfa;
                comp[144] = 0x58;
                comp[145] = 0x80;
                comp[146] = 0x8c;
                comp[147] = 0x22;
                comp[148] = 0xc4;
                comp[149] = 0x13;

                // Modified Time: 0x13c4228cbbf3c400
                comp[150] = 0x00;
                comp[151] = 0xc4;
                comp[152] = 0xf3;
                comp[153] = 0xbb;
                comp[154] = 0x8c;
                comp[155] = 0x22;
                comp[156] = 0xc4;
                comp[157] = 0x13;

                // Creation Time: 0x13c4228cf78e8e00
                comp[158] = 0x00;
                comp[159] = 0x8e;
                comp[160] = 0x8e;
                comp[161] = 0xf7;
                comp[162] = 0x8c;
                comp[163] = 0x22;
                comp[164] = 0xc4;
                comp[165] = 0x13;

                // Checksum
                comp[166] = 35;

                // indirect pointer
                comp[167] = 0;

                comp[175] = 0x1; // Size

                // blocks
                comp[176] = 0x2;

                comp[184] = 0x56;
                comp[185] = 0x34;
                comp[186] = 0x12;

                comp
            };

            let mut blocks = [Extent::zeroed(); 5];
            blocks[0].start = 0x2;
            blocks[0].end = 0x123456;

            let node = INode::new(
                1,
                "name",
                246,
                INodeFlags::new(true, true, false, false),
                DateTime::from(
                    DateTime::parse_from_rfc2822("Wed, 18 Feb 2015 23:16:09 +0000").unwrap(),
                ),
                DateTime::from(
                    DateTime::parse_from_rfc2822("Wed, 18 Feb 2015 23:16:10 +0000").unwrap(),
                ),
                DateTime::from(
                    DateTime::parse_from_rfc2822("Wed, 18 Feb 2015 23:16:11 +0000").unwrap(),
                ),
                0,
                1,
                blocks,
            );

            assert!(node.perform_checksum());

            assert_eq!(INode::from_bytes(&comp).unwrap(), node);
        }

        #[test]
        fn test_to_from_bytes() {
            let mut blocks = [Extent::zeroed(); 5];
            blocks[0].start = 0x2;
            blocks[0].end = 0x123456;

            let node = INode::new(
                1,
                "name",
                246,
                INodeFlags::new(true, true, false, false),
                DateTime::from(
                    DateTime::parse_from_rfc2822("Wed, 18 Feb 2015 23:16:09 +0000").unwrap(),
                ),
                DateTime::from(
                    DateTime::parse_from_rfc2822("Thu, 19 Feb 2015 23:16:10 +0000").unwrap(),
                ),
                DateTime::from(
                    DateTime::parse_from_rfc2822("Fri, 20 Feb 2015 23:16:11 +0000").unwrap(),
                ),
                0,
                1,
                blocks,
            );

            assert!(node.perform_checksum());

            assert_eq!(INode::from_bytes(&node.to_bytes()).unwrap(), node);
        }
    }

    mod indirect_inode {
        use super::*;

        #[test]
        fn test_to_bytes() {
            let extents = {
                let mut pointers = vec![Extent::zeroed()];

                pointers[0].start = 0xdf627312;
                pointers[0].end = 0xef627312;

                pointers
            };

            let node = IndirectINode::new(extents, 0xfeff12, 4096);
            assert!(node.perform_checksum());

            let comp = {
                let mut comp = vec![0u8; IndirectINode::NON_EXPANDABLE_SIZE as usize];

                // Checksum
                comp[0] = 84;

                // Reserved
                comp[1] = 0;

                // Next
                comp[2] = 0x12;
                comp[3] = 0xff;
                comp[4] = 0xfe;

                // Number of extents
                comp[10] = 1;

                // Pointers
                comp.push(0x12);
                comp.push(0x73);
                comp.push(0x62);
                comp.push(0xdf);
                comp.push(0);
                comp.push(0);
                comp.push(0);
                comp.push(0);

                comp.push(0x12);
                comp.push(0x73);
                comp.push(0x62);
                comp.push(0xef);
                comp.push(0);
                comp.push(0);
                comp.push(0);
                comp.push(0);

                comp
            };

            let bytes = node.to_bytes();

            assert_eq!(comp, bytes.to_vec());
        }

        #[test]
        fn test_from_bytes() {
            let comp = {
                let mut comp = vec![0u8; IndirectINode::NON_EXPANDABLE_SIZE as usize];

                // Checksum
                comp[0] = 84;

                // Reserved
                comp[1] = 0;

                // Next
                comp[2] = 0x12;
                comp[3] = 0xff;
                comp[4] = 0xfe;

                // Number of extents
                comp[10] = 1;

                // Pointers
                comp.push(0x12);
                comp.push(0x73);
                comp.push(0x62);
                comp.push(0xdf);
                comp.push(0);
                comp.push(0);
                comp.push(0);
                comp.push(0);

                comp.push(0x12);
                comp.push(0x73);
                comp.push(0x62);
                comp.push(0xef);
                comp.push(0);
                comp.push(0);
                comp.push(0);
                comp.push(0);

                comp
            };

            let extents = {
                let mut pointers = vec![Extent::zeroed()];

                pointers[0].start = 0xdf627312;
                pointers[0].end = 0xef627312;

                pointers
            };

            let node = IndirectINode::new(extents, 0xfeff12, 4096);
            assert!(node.perform_checksum());

            let mut comp = IndirectINode::from_bytes(&comp).unwrap();
            comp.maximum_extents = node.maximum_extents;
            assert!(comp.perform_checksum());

            assert_eq!(node, comp);
        }

        #[test]
        fn test_to_from_bytes() {
            let extents = {
                let mut pointers = vec![Extent::zeroed()];

                pointers[0].start = 0xdf627312;
                pointers[0].end = 0xef627312;

                pointers
            };

            let node = IndirectINode::new(extents, 0xfeff12, 4096);
            assert!(node.perform_checksum());

            let mut comp = IndirectINode::from_bytes(&node.to_bytes()).unwrap();
            assert!(comp.perform_checksum());
            comp.maximum_extents = node.maximum_extents; // This is

            assert_eq!(comp, node);
        }
    }
}
