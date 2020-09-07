use crate::{ByteSerializable, Checksum};
use alloc::vec::Vec;
use byteorder::{ByteOrder, LittleEndian};
use chrono::{DateTime, Utc};

#[derive(Clone, Copy)]
/// Length of 256 bytes
pub struct TagBlock {
    /// index, the index of the tag block in the map.
    index: u64,
    /// The name of this tag.
    name: [char; 132],
    /// Checksum
    checksum: u8,
    /// Flags
    flags: TagFlags,
    /// creation time, nano seconds since unix epoch
    creation_time: u64,
    /// A pointer to a data block that contains more pointers to files, this should be an address not an index
    indirect: u64,
    /// Pointers to inodes that are contained. This is the number contained in just this block.
    number_of_pointers: u16,
    /// member files, represented by indexes in the inode data map.
    members: [u64; 12],
}

#[derive(Eq, PartialEq, Clone, Copy, Debug)]
pub struct TagFlags {
    read: bool,
    write: bool,
    // bits 3-8 are reserved
}

// Size of 1 block
#[derive(Clone, PartialEq, Eq)]
pub struct IndirectTagBlock {
    /// The index of the TagBlock
    root: u64,
    /// Checksum
    checksum: u8,
    /// Reserved byte for potential future use.
    reserved: u8,
    /// Pointer to another IndirectTagBlock, the physical address NOT an index.
    next: u64,
    /// The number of members stored in this block only.
    number_of_members: u16,
    /// member files, represented by indexes in the inode data map.
    members: Vec<u64>,

    /// This is NOT serialized or placed on disk, it is for the methods to know the limit for the number of members.
    pub maximum_members: u64,
}

impl TagBlock {
    pub const MAXIMUM_LOCAL_MEMBERS: u16 = 12;
    pub const MAX_NAME_LENGTH: usize = 132;

    pub fn new(
        index: u64,
        name_str: &str,
        flags: TagFlags,
        creation_time: DateTime<Utc>,
        indirect: u64,
        number_of_pointers: u16,
        members: [u64; 12],
    ) -> Self {
        return Self::new_custom_creation_time(
            index,
            name_str,
            flags,
            creation_time.timestamp_nanos() as u64,
            indirect,
            number_of_pointers,
            members,
        );
    }

    fn new_custom_creation_time(
        index: u64,
        name_str: &str,
        flags: TagFlags,
        creation_time: u64,
        indirect: u64,
        number_of_pointers: u16,
        members: [u64; 12],
    ) -> Self {
        let mut name = ['\0'; Self::MAX_NAME_LENGTH];

        for (i, ch) in name_str.chars().enumerate() {
            if i >= Self::MAX_NAME_LENGTH {
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

    pub fn indirect_pointer(&self) -> Option<u64> {
        if self.indirect == 0 {
            return None;
        } else {
            return Some(self.indirect);
        }
    }

    pub fn set_indirect(&mut self, indirect: u64) {
        self.indirect = indirect;

        self.set_checksum();
    }

    pub fn set_indirect_optional(&mut self, indirect: Option<u64>) {
        match indirect {
            Some(i) => self.set_indirect(i),
            None => self.set_indirect(0),
        }
    }

    pub fn set_index(&mut self, index: u64) {
        self.index = index;
    }

    pub fn number_of_pointers(&self) -> u16 {
        return self.number_of_pointers;
    }

    pub fn members(&self) -> [u64; 12] {
        return self.members;
    }

    pub fn contains_member(&self, member: &u64) -> bool {
        return self.members[..self.number_of_pointers as usize].contains(member);
    }

    pub fn append_member(&mut self, member: u64) -> bool {
        if self.number_of_pointers == Self::MAXIMUM_LOCAL_MEMBERS {
            return false;
        }

        self.members[self.number_of_pointers as usize] = member;
        self.number_of_pointers += 1;
        self.set_checksum();

        return true;
    }

    pub fn member_at(&self, index: u16) -> u64 {
        return self.members[index as usize];
    }

    pub fn remove_member_at(&mut self, index: u16) -> bool {
        if self.number_of_pointers == 0 {
            return false;
        } else if index < self.number_of_pointers - 1 {
            for i in index..self.number_of_pointers - 1 {
                self.members[i as usize] = self.members[i as usize + 1];
            }
        } else if index != self.number_of_pointers - 1 {
            return false;
        }

        self.members[self.number_of_pointers as usize - 1] = 0;
        self.number_of_pointers -= 1;

        self.set_checksum();

        return true;
    }

    pub fn index(&self) -> u64 {
        return self.index;
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

        LittleEndian::write_u16(&mut res[offset..], self.number_of_pointers);
        offset += 2;

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
        let mut name = ['\0'; Self::MAX_NAME_LENGTH];
        let checksum: u8;
        let flags: TagFlags;
        let creation_time: u64;
        let indirect: u64;
        let number_of_pointers: u16;
        let mut members = [0u64; 12];

        let mut offset = 0;

        index = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;

        for i in 0..Self::MAX_NAME_LENGTH {
            name[i] = bytes[offset + i] as char;
        }

        offset += Self::MAX_NAME_LENGTH;

        checksum = bytes[offset];
        offset += 1;
        flags = TagFlags::from_u8(bytes[offset]);
        offset += 1;

        creation_time = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;
        indirect = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;
        number_of_pointers = LittleEndian::read_u16(&bytes[offset..]);
        offset += 2;

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
        self.checksum = 0; // For the purpose of calculation
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
    /// The size in bytes of the fixed length elements within an indirect Tag.
    const NON_EXPANDABLE_SIZE: u64 = 8 + 1 + 1 + 8 + 2;

    pub fn new(root: u64, members: Vec<u64>, next: u64, block_size: u64) -> Self {
        assert!(members.len() < u16::MAX as usize); // The developer should ensure this.
        let maximum_members = Self::max_members_for_blocksize(block_size);
        assert!(members.len() <= maximum_members as usize); // The developer should ensure this.

        let number_of_members = members.len() as u16;

        let mut res = Self {
            root,
            checksum: 0,
            reserved: 0,
            next,
            number_of_members,
            members,
            maximum_members,
        };

        res.set_checksum();

        return res;
    }

    pub fn members(&self) -> Vec<u64> {
        return self.members.clone();
    }

    pub fn capacity(&self) -> u64 {
        return self.maximum_members;
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

    pub fn set_next_optional(&mut self, next: Option<u64>) {
        match next {
            Some(i) => self.set_next(i),
            None => self.set_next(0),
        }
    }

    /// Returns true if the member is a part of this indirect tag block.
    /// NOTE: It assumes that in the members vector is ONLY valid members.
    pub fn contains_member(&self, member: &u64) -> bool {
        return self.members.contains(member);
    }

    pub fn number_of_members(&self) -> u16 {
        return self.number_of_members;
    }

    pub fn set_block_size(&mut self, block_size: u64) {
        self.maximum_members = Self::max_members_for_blocksize(block_size);
    }

    pub fn append_member(&mut self, member: u64) -> bool {
        if self.number_of_members as u64 >= self.maximum_members {
            return false;
        }

        self.members.push(member);
        self.number_of_members += 1;

        self.set_checksum();

        return true;
    }

    pub fn remove_member_at(&mut self, index: u16) -> bool {
        if self.number_of_members == 0 {
            return false;
        } else if index >= self.number_of_members {
            return false;
        } else {
            self.members.remove(index as usize);
        }

        self.number_of_members -= 1;

        self.set_checksum();

        return true;
    }

    pub fn member_at(&self, index: u16) -> u64 {
        return self.members[index as usize];
    }

    #[inline]
    pub fn max_members_for_blocksize(blocksize: u64) -> u64 {
        return (blocksize - Self::NON_EXPANDABLE_SIZE) / 8;
    }

    pub fn to_bytes_padded(&self, block_size: usize) -> Vec<u8> {
        let mut bytes = self.to_bytes();

        while bytes.len() < block_size {
            bytes.push(0);
        }

        return bytes;
    }
}

impl ByteSerializable for IndirectTagBlock {
    type BytesArrayType = Vec<u8>;

    fn to_bytes(&self) -> Self::BytesArrayType {
        let mut bytes = Vec::new();
        let mut working = [0u8; 8];

        LittleEndian::write_u64(&mut working, self.root);
        bytes.extend_from_slice(&working);

        bytes.push(self.checksum);
        bytes.push(self.reserved);

        LittleEndian::write_u64(&mut working, self.next);
        bytes.extend_from_slice(&working);

        LittleEndian::write_u16(&mut working, self.number_of_members);
        bytes.extend_from_slice(&working[..2]);

        for member in self.members.iter() {
            LittleEndian::write_u64(&mut working, *member);
            bytes.extend_from_slice(&working);
        }

        return bytes;
    }

    fn from_bytes(bytes: &[u8]) -> Option<Self>
    where
        Self: core::marker::Sized,
    {
        if bytes.len() < Self::NON_EXPANDABLE_SIZE as usize {
            return None;
        }

        let root: u64;
        let checksum: u8;
        let reserved: u8;
        let next: u64;
        let number_of_members: u16;
        let mut members = Vec::new();

        let mut offset = 0;

        root = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;

        checksum = bytes[offset];
        offset += 1;

        reserved = bytes[offset];
        offset += 1;

        next = LittleEndian::read_u64(&bytes[offset..]);
        offset += 8;

        number_of_members = LittleEndian::read_u16(&bytes[offset..]);
        offset += 2;

        for _ in 0..number_of_members {
            members.push(LittleEndian::read_u64(&bytes[offset..]));
            offset += 8;
        }

        let res = Self {
            root,
            checksum,
            reserved,
            next,
            number_of_members,
            members,
            maximum_members: 0,
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
        self.checksum = 0; // For the purpose of the calculation.
        self.checksum = self.calculate_checksum();
    }
}

impl core::fmt::Debug for IndirectTagBlock {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        return f
            .debug_struct("IndirectTagBlock")
            .field("root", &self.root)
            .field("checksum", &self.checksum)
            .field("reserved", &self.reserved)
            .field("next", &self.next)
            .field("number_of_members", &self.number_of_members)
            .field("members", &self.members)
            .field("maximum_members", &self.maximum_members)
            .finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

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
            let block = TagBlock::new_custom_creation_time(
                0,
                "files",
                TagFlags::new(true, false),
                0xad23132ad,
                0x0,
                0x1,
                members,
            );

            let mut comp_name = ['\0'; TagBlock::MAX_NAME_LENGTH];
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

            let block = TagBlock::new_custom_creation_time(
                0,
                "files",
                TagFlags::new(true, false),
                0xad23132ad,
                0x0,
                0x1,
                members,
            );

            let block2 = TagBlock::new_custom_creation_time(
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

            let block = TagBlock::new_custom_creation_time(
                0,
                "iles",
                TagFlags::new(true, false),
                0xad23132ad,
                0x0,
                0x1,
                members,
            );

            let block2 = TagBlock::new_custom_creation_time(
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
            let block = TagBlock::new_custom_creation_time(
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
            let mut block = TagBlock::new_custom_creation_time(
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
            let block = TagBlock::new_custom_creation_time(
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
                bytes[140] = 155;

                // Flags
                bytes[141] = 0b1000_0000;

                // creation_time
                bytes[142] = 0xad;
                bytes[143] = 0x32;
                bytes[144] = 0x31;
                bytes[145] = 0xd2;
                bytes[146] = 0xba;

                // indirect
                bytes[150] = 0x1;

                // number of pointers
                bytes[158] = 0x1;

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
            let block = TagBlock::new_custom_creation_time(
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
                bytes[140] = 155;

                // Flags
                bytes[141] = 0b1000_0000;

                // creation_time
                bytes[142] = 0xad;
                bytes[143] = 0x32;
                bytes[144] = 0x31;
                bytes[145] = 0xd2;
                bytes[146] = 0xba;

                // indirect
                bytes[150] = 0x1;

                // number of pointers
                bytes[158] = 0x1;

                //members
                bytes[160] = 0x33;

                bytes
            };

            let comp = TagBlock::from_bytes(&bytes).unwrap();

            assert!(comp.perform_checksum());
            assert_eq!(comp, block);
        }

        #[test]
        fn test_append_first() {
            let members = [0u64; 12];
            let mut comp_members = [0u64; 12];
            comp_members[0] = 3;

            let mut block = TagBlock::new_custom_creation_time(
                0,
                "",
                TagFlags::new(false, false),
                0,
                0,
                0,
                members,
            );
            assert!(block.append_member(3));

            assert_eq!(block.number_of_pointers, 1);
            assert_eq!(block.members, comp_members);
        }

        #[test]
        fn test_append_fail() {
            let members = [3u64; 12];
            let comp_members = [3u64; 12];

            let mut block = TagBlock::new_custom_creation_time(
                0,
                "",
                TagFlags::new(false, false),
                0,
                0,
                12,
                members,
            );
            assert!(!block.append_member(2));

            assert_eq!(block.number_of_pointers, 12);
            assert_eq!(block.members, comp_members);
        }

        #[test]
        fn test_remove_member_at_first() {
            let mut members = [0u64; 12];
            members[0] = 0x33;
            let comp_members = [0u64; 12];

            let mut block = TagBlock::new_custom_creation_time(
                0,
                "",
                TagFlags::new(false, false),
                0,
                0,
                1,
                members,
            );

            assert_eq!(block.members, members);
            assert_eq!(block.number_of_pointers, 1);

            assert!(block.remove_member_at(0));
            assert_eq!(block.members, comp_members);
            assert_eq!(block.number_of_pointers, 0);
        }

        #[test]
        fn test_remove_member_at_append() {
            let members = [0u64; 12];
            let comp_members = [0u64; 12];

            let mut block = TagBlock::new_custom_creation_time(
                0,
                "",
                TagFlags::new(false, false),
                0,
                0,
                0,
                members,
            );

            assert_eq!(block.members, members);
            assert_eq!(block.number_of_pointers, 0);

            block.append_member(23);
            assert!(block.remove_member_at(0));

            assert_eq!(block.members, comp_members);
            assert_eq!(block.number_of_pointers, 0);
        }

        #[test]
        fn test_remove_member_at_fail() {
            let members = [0u64; 12];
            let mut comp_members = [0u64; 12];
            comp_members[0] = 3;

            let mut block = TagBlock::new_custom_creation_time(
                0,
                "",
                TagFlags::new(false, false),
                0,
                0,
                0,
                members,
            );

            assert_eq!(block.members, members);
            assert_eq!(block.number_of_pointers, 0);

            assert!(!block.remove_member_at(0));
            block.append_member(3);
            assert!(!block.remove_member_at(1));

            assert_eq!(block.members, comp_members);
            assert_eq!(block.number_of_pointers, 1);
        }

        #[test]
        fn test_remove_member_at_depth() {
            let mut members = [0u64; 12];
            members[0] = 4;
            members[1] = 2;
            members[2] = 3;
            members[3] = 2;
            members[4] = 1;
            let mut comp_members = [0u64; 12];
            comp_members[0] = 3;
            comp_members[1] = 1;

            let mut block = TagBlock::new_custom_creation_time(
                0,
                "",
                TagFlags::new(false, false),
                0,
                0,
                5,
                members,
            );

            assert_eq!(block.members, members);
            assert_eq!(block.number_of_pointers, 5);

            assert!(block.remove_member_at(1));
            assert!(block.remove_member_at(0));
            assert!(block.remove_member_at(1));

            assert_eq!(block.members, comp_members);
            assert_eq!(block.number_of_pointers, 2);
        }
    }

    mod indirect_tag_block {
        use super::*;

        #[test]
        fn test_new() {
            let mut members = Vec::new();
            members.push(0x33);
            let block = IndirectTagBlock::new(0, members.clone(), 0x032, 4096);

            assert!(block.perform_checksum());

            assert_eq!(
                IndirectTagBlock {
                    root: 0x0,
                    checksum: 154,
                    reserved: 0,
                    next: 0x032,
                    number_of_members: 1,
                    members,
                    maximum_members: 509,
                },
                block
            );
        }

        #[test]
        fn test_to_bytes() {
            let mut members = Vec::new();
            members.push(0x33);

            let block = IndirectTagBlock::new(0xad44, members, 0x32, 4096);

            let comp_bytes = {
                let mut bytes = Vec::new();

                // Root
                bytes.push(0x44);
                bytes.push(0xad);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);

                // Checksum
                bytes.push(169);

                // Reserved
                bytes.push(0);

                // next
                bytes.push(0x32);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);

                // Number of members
                bytes.push(1);
                bytes.push(0);

                // Members
                bytes.push(0x33);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);

                bytes
            };

            assert_eq!(block.to_bytes().to_vec(), comp_bytes);
        }

        #[test]
        fn test_from_bytes() {
            let mut members = Vec::new();
            members.push(0x33);

            let block = IndirectTagBlock::new(0xad44, members, 0x32, 4096);

            let bytes = {
                let mut bytes = Vec::new();

                // Root
                bytes.push(0x44);
                bytes.push(0xad);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);

                // Checksum
                bytes.push(169);

                // Reserved
                bytes.push(0);

                // next
                bytes.push(0x32);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);

                // Number of members
                bytes.push(1);
                bytes.push(0);

                // Members
                bytes.push(0x33);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);
                bytes.push(0);

                bytes
            };

            let mut res = IndirectTagBlock::from_bytes(&bytes).unwrap();
            res.maximum_members = 509;

            assert_eq!(block, res);
        }

        #[test]
        fn test_to_from_bytes() {
            let mut members = Vec::new();
            members.push(0x33);

            let block = IndirectTagBlock::new(0xad44, members, 0x32, 4096);
            let mut res = IndirectTagBlock::from_bytes(&block.to_bytes()).unwrap();
            res.maximum_members = 509;

            assert_eq!(block, res,);
        }

        #[test]
        fn test_append_first() {
            let members = vec![0u64; 12];
            let mut comp_members = vec![0u64; 13];
            comp_members[12] = 3;

            let mut block = IndirectTagBlock::new(0, members, 0, 4096);
            assert!(block.append_member(3));

            assert_eq!(block.number_of_members, 13);
            assert_eq!(block.members, comp_members);
        }

        #[test]
        fn test_append_fail() {
            let members = Vec::new();
            let mut comp_members = Vec::new();

            let mut block = IndirectTagBlock::new(0, members, 0, 4096);

            for i in 0..block.maximum_members {
                assert!(block.append_member(i));
                comp_members.push(i);
            }

            assert!(!block.append_member(3));

            assert_eq!(block.number_of_members, block.maximum_members as u16);
            assert_eq!(block.members, comp_members);
        }

        #[test]
        fn test_remove_member_at_first() {
            let mut members = vec![0u64; 12];
            members[0] = 0x33;
            let comp_members = vec![0u64; 11];

            let mut block = IndirectTagBlock::new(0, members.clone(), 0, 4096);

            assert_eq!(block.members, members);
            assert_eq!(block.number_of_members, 12);

            assert!(block.remove_member_at(0));

            assert_eq!(block.members, comp_members);
            assert_eq!(block.number_of_members, 11);
        }

        #[test]
        fn test_remove_member_at_append() {
            let members = Vec::new();
            let comp_members = Vec::new();

            let mut block = IndirectTagBlock::new(0, members.clone(), 0, 4096);

            assert_eq!(block.members, members);
            assert_eq!(block.number_of_members, 0);

            assert!(block.append_member(23));
            assert!(block.remove_member_at(0));

            assert_eq!(block.members, comp_members);
            assert_eq!(block.number_of_members, 0);
        }

        #[test]
        fn test_remove_member_at_fail() {
            let members = Vec::new();
            let comp_members: Vec<u64> = vec![23];

            let mut block = IndirectTagBlock::new(0, members.clone(), 0, 4096);

            assert_eq!(block.members, members);
            assert_eq!(block.number_of_members, 0);

            assert!(block.append_member(23));
            assert!(!block.remove_member_at(1));

            assert_eq!(block.members, comp_members);
            assert_eq!(block.number_of_members, 1);
        }
    }
}
