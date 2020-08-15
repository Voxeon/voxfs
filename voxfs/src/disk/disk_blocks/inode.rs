use crate::ByteSerializable;
use crate::Checksum;
use alloc::{vec, vec::Vec};
use byteorder::{ByteOrder, LittleEndian};
use chrono::{DateTime, TimeZone, Timelike, Utc};

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
    /// name, 126 bytes constant filled with null bytes otherwise
    name: [char; 126],
    /// size in bytes
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
    /// indirect block points to a block that only contains a list of pointers to other blocks
    indirect_block: u64,
    /// pointers to blocks, if a space is unused it will be represented simply by 0
    blocks: [Extent; 5],
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Extent {
    start: u64,
    end: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// This takes up 512 bytes
pub struct IndirectINode {
    /// The location of the inode
    root: u64,
    /// Checksum
    checksum: u8,
    /// Reserved bytes
    reserved: [u8; 15],
    /// 30 Extents
    pointers: [Extent; 30],
    /// Pointer to another IndirectINode
    next: u64,
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
        blocks: [Extent; 5],
    ) -> Self {
        let mut name: [char; 126] = ['\0'; 126];

        for (i, c) in str_name.chars().enumerate() {
            if i >= 126 {
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
            blocks,
        };

        res.set_checksum();

        return res;
    }

    pub fn size() -> u64 {
        return 256;
    }
}

impl Checksum for INode {
    fn set_checksum(&mut self) {
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

        offset += 126;

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
        let mut name = ['\0'; 126];
        let size: u64;
        let flags: INodeFlags;
        let access_time: u64;
        let modified_time: u64;
        let creation_time: u64;
        let checksum: u8;
        let indirect_block: u64;
        let mut blocks = [Extent::zeroed(); 5];

        index = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;

        let mut i = 0;
        for ch in &bytes[offset..offset + 126] {
            name[i] = *ch as char;
            i += 1;
        }
        offset += 126;

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

        for i in 0..126usize {
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
            .field("blocks", &self.blocks)
            .finish();
    }
}

impl Extent {
    #[inline]
    pub fn zeroed() -> Self {
        return Self { start: 0, end: 0 };
    }
}

impl IndirectINode {
    pub fn new(root: u64, pointers: [Extent; 30], next: u64) -> Self {
        let mut res = Self {
            root,
            checksum: 0,
            reserved: [0u8; 15],
            pointers,
            next,
        };

        res.set_checksum();

        return res;
    }
}

impl ByteSerializable for IndirectINode {
    type BytesArrayType = [u8; (2 + 31 * 2) * 8];

    fn to_bytes(&self) -> Self::BytesArrayType {
        let mut bytes = [0u8; (2 + 31 * 2) * 8];
        let mut offset = 0;

        LittleEndian::write_u64(&mut bytes[offset..], self.root);
        offset += 8;

        bytes[offset] = self.checksum;
        offset += 1;

        offset += 15; // Reserved bytes

        for extent in &self.pointers {
            LittleEndian::write_u64(&mut bytes[offset..], extent.start);
            offset += 8;
            LittleEndian::write_u64(&mut bytes[offset..], extent.end);
            offset += 8;
        }

        LittleEndian::write_u64(&mut bytes[offset..], self.next);
        offset += 8;

        return bytes;
    }

    // Performs a checksum check
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 512 {
            return None;
        }

        let root;
        let checksum;
        let mut extents = [Extent::zeroed(); 30];
        let next;

        let mut offset = 0;

        root = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;

        checksum = bytes[offset];
        offset += 1;

        offset += 15; // Reserved bytes

        for i in 0..30 {
            let start = LittleEndian::read_u64(&bytes[offset..]);
            offset += 8;
            let end = LittleEndian::read_u64(&bytes[offset..]);
            offset += 8;

            extents[i] = Extent { start, end };
        }

        next = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;

        let res = Self {
            root,
            checksum,
            reserved: [0u8; 15],
            pointers: extents,
            next,
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

            comp[134] = 246; // Size
            comp[142] = 0b1100_0000; // Flags

            // Access Time: 0x13c4228c8058fa00
            comp[143] = 0x00;
            comp[144] = 0xfa;
            comp[145] = 0x58;
            comp[146] = 0x80;
            comp[147] = 0x8c;
            comp[148] = 0x22;
            comp[149] = 0xc4;
            comp[150] = 0x13;

            // Modified Time: 0x13c4228cbbf3c400
            comp[151] = 0x00;
            comp[152] = 0xc4;
            comp[153] = 0xf3;
            comp[154] = 0xbb;
            comp[155] = 0x8c;
            comp[156] = 0x22;
            comp[157] = 0xc4;
            comp[158] = 0x13;

            // Creation Time: 0x13c4228cf78e8e00
            comp[159] = 0x00;
            comp[160] = 0x8e;
            comp[161] = 0x8e;
            comp[162] = 0xf7;
            comp[163] = 0x8c;
            comp[164] = 0x22;
            comp[165] = 0xc4;
            comp[166] = 0x13;

            // Checksum
            comp[167] = 36;

            // indirect pointer
            comp[168] = 0;

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

                comp[134] = 246; // Size
                comp[142] = 0b1100_0000; // Flags

                // Access Time: 0x13c4228c8058fa00
                comp[143] = 0x00;
                comp[144] = 0xfa;
                comp[145] = 0x58;
                comp[146] = 0x80;
                comp[147] = 0x8c;
                comp[148] = 0x22;
                comp[149] = 0xc4;
                comp[150] = 0x13;

                // Modified Time: 0x13c4228cbbf3c400
                comp[151] = 0x00;
                comp[152] = 0xc4;
                comp[153] = 0xf3;
                comp[154] = 0xbb;
                comp[155] = 0x8c;
                comp[156] = 0x22;
                comp[157] = 0xc4;
                comp[158] = 0x13;

                // Creation Time: 0x13c4228cf78e8e00
                comp[159] = 0x00;
                comp[160] = 0x8e;
                comp[161] = 0x8e;
                comp[162] = 0xf7;
                comp[163] = 0x8c;
                comp[164] = 0x22;
                comp[165] = 0xc4;
                comp[166] = 0x13;

                // Checksum
                comp[167] = 36;

                // indirect pointer
                comp[168] = 0;

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
                let mut pointers = [Extent::zeroed(); 30];

                pointers[0].start = 0xdf627312;
                pointers[0].end = 0xef627312;

                pointers
            };

            let node = IndirectINode::new(0xadd7e55a, extents, 0xfeff12);
            assert!(node.perform_checksum());

            let mut comp = {
                let mut comp = vec![0u8; 512];

                // Root
                comp[0] = 0x5a;
                comp[1] = 0xe5;
                comp[2] = 0xd7;
                comp[3] = 0xad;

                // Checksum
                comp[8] = 146;

                // Pointers
                comp[24] = 0x12;
                comp[25] = 0x73;
                comp[26] = 0x62;
                comp[27] = 0xdf;

                comp[32] = 0x12;
                comp[33] = 0x73;
                comp[34] = 0x62;
                comp[35] = 0xef;

                // Next
                comp[504] = 0x12;
                comp[505] = 0xff;
                comp[506] = 0xfe;

                comp
            };

            let bytes = node.to_bytes();

            assert_eq!(comp, bytes.to_vec());
        }

        #[test]
        fn test_from_bytes() {
            let mut comp = {
                let mut comp = vec![0u8; 512];

                // Root
                comp[0] = 0x5a;
                comp[1] = 0xe5;
                comp[2] = 0xd7;
                comp[3] = 0xad;

                // Checksum
                comp[8] = 146;

                // Pointers
                comp[24] = 0x12;
                comp[25] = 0x73;
                comp[26] = 0x62;
                comp[27] = 0xdf;

                comp[32] = 0x12;
                comp[33] = 0x73;
                comp[34] = 0x62;
                comp[35] = 0xef;

                // Next
                comp[504] = 0x12;
                comp[505] = 0xff;
                comp[506] = 0xfe;

                comp
            };

            let extents = {
                let mut pointers = [Extent::zeroed(); 30];

                pointers[0].start = 0xdf627312;
                pointers[0].end = 0xef627312;

                pointers
            };

            let node = IndirectINode::new(0xadd7e55a, extents, 0xfeff12);
            assert!(node.perform_checksum());

            let comp = IndirectINode::from_bytes(&comp).unwrap();
            assert!(comp.perform_checksum());

            assert_eq!(node, comp);
        }

        #[test]
        fn test_to_from_bytes() {
            let extents = {
                let mut pointers = [Extent::zeroed(); 30];

                pointers[0].start = 0xdf627312;
                pointers[0].end = 0xef627312;

                pointers
            };

            let node = IndirectINode::new(0xadd7e55a, extents, 0xfeff12);
            assert!(node.perform_checksum());

            let comp = IndirectINode::from_bytes(&node.to_bytes()).unwrap();
            assert!(comp.perform_checksum());

            assert_eq!(comp, node);
        }
    }
}
