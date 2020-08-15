use crate::{ByteSerializable, Checksum};
use byteorder::{ByteOrder, LittleEndian};
use chrono::format::Item::Literal;

#[derive(Clone, Copy)]
/// Length of 256 bytes
pub struct TagBlock {
    /// index,  the index of the tag block in the map.
    index: u64,
    /// The name of this tag.
    name: [char; 126],
    /// Checksum
    checksum: u8,
    /// Flags
    flags: TagFlags,
    /// creation time, nano seconds since unix epoch
    creation_time: u64,
    /// A pointer to a data block that contains more pointers to files.
    indirect: u64,
    /// Pointers to inodes that are contained. This is the TOTAL number including the referenced blocks.
    number_of_pointers: u64,
    /// member files
    members: [u64; 12],
}

#[derive(Eq, PartialEq, Clone, Copy, Debug)]
pub struct TagFlags {
    read: bool,
    write: bool,
    // bits 3-8 are reserved
}

// Size of 512 bytes
#[derive(Clone, Copy)]
pub struct IndirectTagBlock {
    /// The location of the TagBlock
    root: u64,
    /// Checksum
    checksum: u8,
    /// Reserved
    reserved: [u8; 7],
    /// member files
    members: [u64; 61],
    /// Pointer to another IndirectTagBlock
    next: u64,
}

impl TagBlock {
    pub fn new(
        index: u64,
        name_str: &str,
        flags: TagFlags,
        creation_time: u64,
        indirect: u64,
        number_of_pointers: u64,
        members: [u64; 12],
    ) -> Self {
        let mut name = ['\0'; 126];

        for (i, ch) in name_str.chars().enumerate() {
            if i >= 126 {
                break;
            }

            name[i] = ch;
        }

        let mut res = Self {
            index,
            name,
            checksum: 0,
            flags,
            creation_time,
            indirect,
            number_of_pointers,
            members,
        };

        res.set_checksum();

        return res;
    }

    pub fn size() -> u64 {
        return 256;
    }
}

impl ByteSerializable for TagBlock {
    type BytesArrayType = [u8; 256];

    fn to_bytes(&self) -> Self::BytesArrayType {
        let mut res = [0u8; 256];
        let mut offset = 0;

        LittleEndian::write_u64(&mut res[offset..], self.index);
        offset += 8;

        for ch in self.name.iter() {
            res[offset] = *ch as u8;
            offset += 1;
        }

        res[offset] = self.checksum;
        offset += 1;

        res[offset] = self.flags.as_u8();
        offset += 1;

        LittleEndian::write_u64(&mut res[offset..], self.creation_time);
        offset += 8;

        LittleEndian::write_u64(&mut res[offset..], self.indirect);
        offset += 8;

        LittleEndian::write_u64(&mut res[offset..], self.number_of_pointers);
        offset += 8;

        for member in self.members.iter() {
            LittleEndian::write_u64(&mut res[offset..], *member);
            offset += 8;
        }

        return res;
    }

    fn from_bytes(bytes: &[u8]) -> Option<Self>
    where
        Self: core::marker::Sized,
    {
        if bytes.len() < 256 {
            return None;
        }

        let index: u64;
        let mut name = ['\0'; 126];
        let checksum: u8;
        let flags: TagFlags;
        let creation_time: u64;
        let indirect: u64;
        let number_of_pointers: u64;
        let mut members = [0u64; 12];

        let mut offset = 0;

        index = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;

        for i in 0..126 {
            name[i] = bytes[offset + i] as char;
        }

        offset += 126;

        checksum = bytes[offset];
        offset += 1;
        flags = TagFlags::from_u8(bytes[offset]);
        offset += 1;

        creation_time = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;
        indirect = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;
        number_of_pointers = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;

        for i in 0..12 {
            members[i] = LittleEndian::read_u64(&bytes[offset..]);
            offset += 8;
        }

        let res = Self {
            index,
            name,
            checksum,
            flags,
            creation_time,
            indirect,
            number_of_pointers,
            members,
        };

        if !res.perform_checksum() {
            return None;
        }

        return Some(res);
    }

    fn generic_bytes_rep(bytes: &Self::BytesArrayType) -> &[u8] {
        return bytes;
    }
}

impl Checksum for TagBlock {
    fn set_checksum(&mut self) {
        self.checksum = self.calculate_checksum();
    }
}

impl core::fmt::Debug for TagBlock {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let name_str: alloc::string::String = self.name.iter().collect();
        let members = self.members.to_vec();
        return f
            .debug_struct("TagBlock")
            .field("index", &self.index)
            .field("name", &name_str)
            .field("checksum", &self.checksum)
            .field("flags", &self.flags)
            .field("creation_time", &self.creation_time)
            .field("indirect", &self.indirect)
            .field("number_of_pointers", &self.number_of_pointers)
            .field("members", &members)
            .finish();
    }
}

impl core::cmp::PartialEq for TagBlock {
    fn eq(&self, other: &Self) -> bool {
        let mut name_comp;
        let mut members_comp;

        // These should be the same but safeguards against future changes
        if self.name.len() != other.name.len() {
            name_comp = false;
        } else {
            name_comp = true;

            for i in 0..self.name.len() {
                if self.name[i] != other.name[i] {
                    name_comp = false;
                    break;
                }
            }
        }

        if self.members.len() != other.members.len() {
            members_comp = false;
        } else {
            members_comp = true;

            for i in 0..self.members.len() {
                if self.members[i] != other.members[i] {
                    members_comp = false;
                    break;
                }
            }
        }

        return self.index == other.index
            && name_comp
            && self.checksum == other.checksum
            && self.flags == other.flags
            && self.creation_time == other.creation_time
            && self.indirect == other.indirect
            && self.number_of_pointers == other.number_of_pointers
            && members_comp;
    }
}

impl TagFlags {
    pub fn new(read: bool, write: bool) -> Self {
        return Self { read, write };
    }

    pub fn as_u8(&self) -> u8 {
        let mut result = 0;

        if self.read {
            result |= 0b1000_0000;
        }

        if self.write {
            result |= 0b0100_0000;
        }

        return result;
    }

    pub fn from_u8(n: u8) -> Self {
        let read = (n >> 7) & 1 == 1;
        let write = (n >> 6) & 1 == 1;

        return Self::new(read, write);
    }
}

impl IndirectTagBlock {
    pub fn new(root: u64, members: [u64; 61], next: u64) -> Self {
        let mut res = Self {
            root,
            checksum: 0,
            reserved: [0u8; 7],
            members,
            next,
        };

        res.set_checksum();

        return res;
    }
}

impl ByteSerializable for IndirectTagBlock {
    type BytesArrayType = [u8; 512];

    fn to_bytes(&self) -> Self::BytesArrayType {
        let mut bytes = [0u8; 512];
        let mut offset = 0;

        LittleEndian::write_u64(&mut bytes[offset..], self.root);
        offset += 8;

        bytes[offset] = self.checksum;
        offset += 1;

        for byte in &self.reserved {
            bytes[offset] = *byte;
            offset += 1;
        }

        for member in self.members.iter() {
            LittleEndian::write_u64(&mut bytes[offset..], *member);
            offset += 8;
        }

        LittleEndian::write_u64(&mut bytes[offset..], self.next);
        offset += 8;

        return bytes;
    }

    fn from_bytes(bytes: &[u8]) -> Option<Self>
    where
        Self: core::marker::Sized,
    {
        if bytes.len() < 512 {
            return None;
        }

        let root: u64;
        let checksum: u8;
        let mut reserved = [0u8; 7];
        let mut members = [0u64; 61];
        let next: u64;

        let mut offset = 0;

        root = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;

        checksum = bytes[offset];
        offset += 1;

        for i in 0..7 {
            reserved[i] = bytes[offset];
            offset += 1;
        }

        for i in 0..61 {
            members[i] = LittleEndian::read_u64(&bytes[offset..]);
            offset += 8;
        }

        next = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;

        let res = Self {
            root,
            checksum,
            reserved,
            members,
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

impl Checksum for IndirectTagBlock {
    fn set_checksum(&mut self) {
        self.checksum = self.calculate_checksum();
    }
}

impl core::fmt::Debug for IndirectTagBlock {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let members = self.members.to_vec();
        return f
            .debug_struct("IndirectTagBlock")
            .field("root", &self.root)
            .field("checksum", &self.checksum)
            .field("members", &members)
            .field("next", &self.next)
            .finish();
    }
}

impl core::cmp::PartialEq for IndirectTagBlock {
    fn eq(&self, other: &Self) -> bool {
        let mut members_comp;

        // These should be the same but safeguards against future changes
        if self.members.len() != other.members.len() {
            members_comp = false;
        } else {
            members_comp = true;

            for i in 0..self.members.len() {
                if self.members[i] != other.members[i] {
                    members_comp = false;
                    break;
                }
            }
        }

        return self.root == other.root
            && self.checksum == other.checksum
            && self.reserved == other.reserved
            && self.next == other.next
            && members_comp;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod tag_flags {
        use super::*;

        #[test]
        fn test_as_u8() {
            let flags = TagFlags::new(true, true);
            assert_eq!(flags.as_u8(), 0b1100_0000);
        }

        #[test]
        fn test_as_u8_2() {
            let flags = TagFlags::new(false, true);
            assert_eq!(flags.as_u8(), 0b0100_0000);
        }

        #[test]
        fn test_as_u8_3() {
            let flags = TagFlags::new(true, false);
            assert_eq!(flags.as_u8(), 0b1000_0000);
        }

        #[test]
        fn test_from_u8() {
            let flags = TagFlags::new(true, true);
            assert_eq!(TagFlags::from_u8(0b1100_0000), flags);
        }

        #[test]
        fn test_from_u8_2() {
            let flags = TagFlags::new(false, true);
            assert_eq!(TagFlags::from_u8(0b0100_0000), flags);
        }

        #[test]
        fn test_from_u8_3() {
            let flags = TagFlags::new(true, false);
            assert_eq!(TagFlags::from_u8(0b1000_0000), flags);
        }
    }

    mod tag_block {
        use super::*;

        #[test]
        fn test_new() {
            let mut members = [0u64; 12];
            members[0] = 0x33;
            let block = TagBlock::new(
                0,
                "files",
                TagFlags::new(true, false),
                0xad23132ad,
                0x0,
                0x1,
                members,
            );

            let mut comp_name = ['\0'; 126];
            comp_name[0] = 'f';
            comp_name[1] = 'i';
            comp_name[2] = 'l';
            comp_name[3] = 'e';
            comp_name[4] = 's';

            assert!(block.perform_checksum());

            assert_eq!(
                TagBlock {
                    index: 0,
                    name: comp_name,
                    checksum: 77,
                    flags: TagFlags::new(true, false),
                    creation_time: 0xad23132ad,
                    indirect: 0x0,
                    number_of_pointers: 0x1,
                    members
                },
                block
            );
        }

        #[test]
        fn test_eq() {
            let mut members = [0u64; 12];
            members[0] = 0x33;

            let block = TagBlock::new(
                0,
                "files",
                TagFlags::new(true, false),
                0xad23132ad,
                0x0,
                0x1,
                members,
            );

            let block2 = TagBlock::new(
                0,
                "files",
                TagFlags::new(true, false),
                0xad23132ad,
                0x0,
                0x1,
                members,
            );

            assert_eq!(block, block2);
        }

        #[test]
        fn test_neq() {
            let mut members = [0u64; 12];
            members[0] = 0x32;

            let block = TagBlock::new(
                0,
                "iles",
                TagFlags::new(true, false),
                0xad23132ad,
                0x0,
                0x1,
                members,
            );

            let block2 = TagBlock::new(
                0,
                "files",
                TagFlags::new(true, false),
                0xad23132ad,
                0x0,
                0x1,
                members,
            );

            assert_ne!(block, block2);
        }

        #[test]
        fn test_checksum_success() {
            let mut members = [0u64; 12];
            members[0] = 0x33;
            let block = TagBlock::new(
                1,
                "files",
                TagFlags::new(true, false),
                0xbad23132ad,
                0x1,
                0x1,
                members,
            );

            assert!(block.perform_checksum());
        }

        #[test]
        fn test_checksum_fail() {
            let mut members = [0u64; 12];
            members[0] = 0x33;
            let mut block = TagBlock::new(
                1,
                "files",
                TagFlags::new(true, false),
                0xbad23132ad,
                0x1,
                0x1,
                members,
            );

            assert!(block.perform_checksum());

            block.indirect += 1;
            assert!(!block.perform_checksum());
        }

        #[test]
        fn test_to_bytes() {
            let mut members = [0u64; 12];
            members[0] = 0x33;
            let block = TagBlock::new(
                1,
                "files",
                TagFlags::new(true, false),
                0xbad23132ad,
                0x1,
                0x1,
                members,
            );

            let comp_bytes = {
                let mut bytes = [0u8; 256];
                bytes[0] = 0x1;

                // name
                bytes[8] = b'f';
                bytes[9] = b'i';
                bytes[10] = b'l';
                bytes[11] = b'e';
                bytes[12] = b's';

                // checksum
                bytes[134] = 155;

                // Flags
                bytes[135] = 0b1000_0000;

                // creation_time
                bytes[136] = 0xad;
                bytes[137] = 0x32;
                bytes[138] = 0x31;
                bytes[139] = 0xd2;
                bytes[140] = 0xba;

                // indirect
                bytes[144] = 0x1;

                // number of pointers
                bytes[152] = 0x1;

                //members
                bytes[160] = 0x33;

                bytes
            };

            assert_eq!(comp_bytes.to_vec(), block.to_bytes().to_vec());
        }

        #[test]
        fn test_from_bytes() {
            let mut members = [0u64; 12];
            members[0] = 0x33;
            let block = TagBlock::new(
                1,
                "files",
                TagFlags::new(true, false),
                0xbad23132ad,
                0x1,
                0x1,
                members,
            );

            let bytes = {
                let mut bytes = [0u8; 256];
                bytes[0] = 0x1;

                // name
                bytes[8] = b'f';
                bytes[9] = b'i';
                bytes[10] = b'l';
                bytes[11] = b'e';
                bytes[12] = b's';

                // checksum
                bytes[134] = 155;

                // Flags
                bytes[135] = 0b1000_0000;

                // creation_time
                bytes[136] = 0xad;
                bytes[137] = 0x32;
                bytes[138] = 0x31;
                bytes[139] = 0xd2;
                bytes[140] = 0xba;

                // indirect
                bytes[144] = 0x1;

                // number of pointers
                bytes[152] = 0x1;

                //members
                bytes[160] = 0x33;

                bytes
            };

            let comp = TagBlock::from_bytes(&bytes).unwrap();

            assert!(comp.perform_checksum());
            assert_eq!(comp, block);
        }
    }

    mod indirect_tag_block {
        use super::*;

        #[test]
        fn test_new() {
            let mut members = [0u64; 61];
            members[0] = 0x33;
            let block = IndirectTagBlock::new(0, members, 0x032);

            assert!(block.perform_checksum());

            assert_eq!(
                IndirectTagBlock {
                    root: 0x0,
                    checksum: 155,
                    reserved: [0u8; 7],
                    members,
                    next: 0x032,
                },
                block
            );
        }

        #[test]
        fn test_eq() {
            let mut members = [0u64; 61];
            members[0] = 0x33;
            let block = IndirectTagBlock::new(0, members, 0x032);

            let block2 = IndirectTagBlock::new(0, members, 0x032);

            assert_eq!(block, block2);
        }

        #[test]
        fn test_neq() {
            let mut members = [0u64; 61];
            members[0] = 0x33;
            let block = IndirectTagBlock::new(0, members, 0x032);

            let block2 = IndirectTagBlock::new(0x456, members, 0x032);

            assert_ne!(block, block2);
        }

        #[test]
        fn test_to_bytes() {
            let mut members = [0u64; 61];
            members[0] = 0x33;
            members[60] = 0xadf3ccbb;

            let block = IndirectTagBlock::new(0xad44, members, 0x32);

            let comp_bytes = {
                let mut bytes = [0u8; 512];

                bytes[0] = 0x44;
                bytes[1] = 0xad;

                bytes[8] = 131;

                bytes[16] = 0x33;
                bytes[496] = 0xbb;
                bytes[497] = 0xcc;
                bytes[498] = 0xf3;
                bytes[499] = 0xad;

                bytes[504] = 0x32;

                bytes
            };

            assert_eq!(block.to_bytes().to_vec(), comp_bytes.to_vec());
        }

        #[test]
        fn test_from_bytes() {
            let mut members = [0u64; 61];
            members[0] = 0x33;
            members[60] = 0xadf3ccbb;

            let block = IndirectTagBlock::new(0xad44, members, 0x32);

            let bytes = {
                let mut bytes = [0u8; 512];

                bytes[0] = 0x44;
                bytes[1] = 0xad;

                bytes[8] = 131;

                bytes[16] = 0x33;
                bytes[496] = 0xbb;
                bytes[497] = 0xcc;
                bytes[498] = 0xf3;
                bytes[499] = 0xad;

                bytes[504] = 0x32;

                bytes
            };

            assert_eq!(block, IndirectTagBlock::from_bytes(&bytes).unwrap());
        }

        #[test]
        fn test_to_from_bytes() {
            let mut members = [0u64; 61];
            members[0] = 0x33;
            members[60] = 0xadf3ccbb;

            let block = IndirectTagBlock::new(0xad44, members, 0x32);

            assert_eq!(
                block,
                IndirectTagBlock::from_bytes(&block.to_bytes()).unwrap()
            );
        }
    }
}
