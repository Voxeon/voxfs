use crate::utils::rightmost_unset_bit;
use alloc::{vec, vec::Vec};
use core::ops::Index;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct BitMap {
    vc: Vec<u64>,
}

impl BitMap {
    /// Constructs a new bitmap with a maximum size, defaulted to all 0's
    pub fn new(size: usize) -> Self {
        let vec_length = {
            if size % 64 != 0 {
                size / 64 + 1
            } else {
                size / 64
            }
        };

        return Self {
            vc: vec![0; vec_length],
        };
    }

    /// Constructs a new bitmap from a sequence of bytes.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut m = Self::new(bytes.len() * 8);
        m.fill_from_bytes(bytes);

        return m;
    }

    /// Find the first free bit and return the index
    pub fn find_next_0_index(&self) -> Option<usize> {
        for (i, val) in self.vc.iter().enumerate() {
            if *val < u64::MAX {
                return Some(i * 64 + rightmost_unset_bit(*val));
            }
        }

        return None;
    }

    /// Fills the buffer using a sequence of bytes.
    fn fill_from_bytes(&mut self, bytes: &[u8]) {
        let mut index = 0;

        for b in 0..bytes.len() / 8 {
            let i = b * 8;

            let mut n = (bytes[i] as u64);

            for p in 1..8 {
                n |= (bytes[i + p] as u64) << (p * 8);
            }

            self.vc[index] = n;
            index += 1;
        }
    }

    /// Tries to set a bit at index and returns true or false if this was done, if it fails increase the size
    pub fn set_bit(&mut self, index: usize, value: bool) -> bool {
        let (array_index, bit) = (index / 64, index % 64);

        if array_index >= self.vc.len() {
            return false;
        }

        if value {
            self.vc[array_index] |= 1 << bit;
        } else {
            self.vc[array_index] ^= 1 << bit;
        }

        return true;
    }

    /// Sets every bit to be either high or low.
    pub fn set_all(&mut self, value: bool) {
        for i in 0..self.vc.len() {
            if value {
                self.vc[i] = u64::MAX;
            } else {
                self.vc[i] = 0;
            }
        }
    }

    /// Returns whether a bit at a specified index was set.
    pub fn bit_at(&self, index: usize) -> Option<bool> {
        let (array_index, bit) = (index / 64, index % 64);

        if array_index >= self.vc.len() {
            return None;
        }

        let value = self.vc[array_index];
        let b = ((value >> bit) & 1);

        return Some(b == 1);
    }

    /// Returns the length. NOTE: The length will ALWAYS be a multiple of 164
    pub fn len(&self) -> usize {
        return self.vc.len() * 64;
    }

    /// Returns the bitmap as a sequence of booleans
    pub fn flatten_bool(&self) -> Vec<bool> {
        let mut res = Vec::new();

        for n in &self.vc {
            for i in 0..64 {
                res.push(((n >> i) & 1) == 1);
            }
        }

        return res;
    }

    /// Returns the bitmap as a sequence of bytes
    pub fn as_bytes(&self) -> Vec<u8> {
        let mut res = Vec::new();

        for n in &self.vc {
            for i in 0..(64 / 8) {
                let x = i * 8;
                let mut r: u8 = ((n >> x) & 1) as u8;

                for offset in 1..8u8 {
                    r |= (((n >> (x + offset)) & 1) as u8) << offset;
                }

                res.push(r);
            }
        }

        return res;
    }

    pub fn count_ones(&self) -> usize {
        let mut sum: usize = 0;

        for n in &self.vc {
            sum += n.count_ones() as usize;
        }

        return sum;
    }

    pub fn count_zeros(&self) -> usize {
        let mut sum: usize = 0;

        for n in &self.vc {
            sum += n.count_zeros() as usize;
        }

        return sum;
    }

    pub fn count_zeros_up_to(&self, index: usize) -> Option<usize> {
        if index >= self.len() {
            return None;
        }

        let chunks = index / 64;
        let chunk_index = index % 64;
        let mut sum: usize = 0;

        for n in 0..chunks {
            sum += self.vc[n].count_zeros() as usize;
        }

        if chunk_index != 0 {
            for i in 0..chunk_index {
                if !self.bit_at(chunks + i).unwrap() {
                    sum += 1;
                }
            }
        }

        return Some(sum);
    }
}

impl core::iter::IntoIterator for BitMap {
    type Item = bool;
    type IntoIter = alloc::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        return self.flatten_bool().into_iter();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bit_set() {
        let mut map = BitMap::new(1024);

        assert!(map.set_bit(3, true));
    }

    #[test]
    fn test_bit_get() {
        let mut map = BitMap::new(1024);

        assert!(map.set_bit(3, true));
        assert_eq!(map.bit_at(3).unwrap(), true); // bit 3 should be set
    }

    #[test]
    fn test_bit_get_2() {
        let mut map = BitMap::new(1024);

        assert!(map.set_bit(3, true));
        assert_eq!(map.bit_at(3).unwrap(), true); // bit 3 should be set
        assert!(map.set_bit(3, false));
        assert_eq!(map.bit_at(3).unwrap(), false); // bit 3 should be set
    }

    #[test]
    fn test_bit_get_3() {
        let mut map = BitMap::new(1024);

        assert!(map.set_bit(342, true));
        assert_eq!(map.bit_at(342).unwrap(), true);
        assert!(map.set_bit(342, false));
        assert_eq!(map.bit_at(342).unwrap(), false);
    }

    #[test]
    fn test_flatten_bool() {
        let mut map = BitMap::new(1024);

        assert!(map.set_bit(342, true));
        assert_eq!(map.bit_at(342).unwrap(), true);

        let mut comp = vec![false; 1024];
        comp[342] = true;

        assert_eq!(map.flatten_bool(), comp);
    }

    #[test]
    fn test_flatten() {
        let mut map = BitMap::new(1024);

        assert!(map.set_bit(342, true));
        assert_eq!(map.bit_at(342).unwrap(), true);

        let mut comp = vec![0; 1024 / 8];
        comp[42] = 1 << 6;

        assert_eq!(map.as_bytes(), comp);
    }

    #[test]
    fn test_flatten_2() {
        let mut map = BitMap::new(1024);

        assert!(map.set_bit(0, true));
        assert_eq!(map.bit_at(0).unwrap(), true);
        assert!(map.set_bit(1, true));
        assert_eq!(map.bit_at(1).unwrap(), true);
        assert!(map.set_bit(8, true));
        assert_eq!(map.bit_at(8).unwrap(), true);
        assert!(map.set_bit(9, true));
        assert_eq!(map.bit_at(9).unwrap(), true);

        let mut comp = vec![0; 1024 / 8];
        comp[0] = 0b11;
        comp[1] = 0b11;

        assert_eq!(map.as_bytes(), comp);
    }

    #[test]
    fn test_from_bytes() {
        let mut map = BitMap::new(1024);

        assert!(map.set_bit(0, true));
        assert_eq!(map.bit_at(0).unwrap(), true);
        assert!(map.set_bit(1, true));
        assert_eq!(map.bit_at(1).unwrap(), true);
        assert!(map.set_bit(8, true));
        assert_eq!(map.bit_at(8).unwrap(), true);
        assert!(map.set_bit(9, true));
        assert_eq!(map.bit_at(9).unwrap(), true);

        let mut comp = vec![0; 1024 / 8];
        comp[0] = 0b11;
        comp[1] = 0b11;

        assert_eq!(map.as_bytes(), comp);
        assert_eq!(BitMap::from_bytes(&comp), map);
    }

    #[test]
    fn test_iter() {
        let mut map = BitMap::new(128);

        assert!(map.set_bit(0, true));
        assert_eq!(map.bit_at(0).unwrap(), true);
        assert!(map.set_bit(1, true));
        assert_eq!(map.bit_at(1).unwrap(), true);
        assert!(map.set_bit(8, true));
        assert_eq!(map.bit_at(8).unwrap(), true);
        assert!(map.set_bit(9, true));
        assert_eq!(map.bit_at(9).unwrap(), true);

        for (index, bit) in map.into_iter().enumerate() {
            if index == 0 || index == 1 || index == 8 || index == 9 {
                assert!(bit);
            } else {
                assert!(!bit);
            }
        }
    }

    #[test]
    fn test_count_ones() {
        let mut map = BitMap::new(1024);

        assert!(map.set_bit(0, true));
        assert_eq!(map.bit_at(0).unwrap(), true);
        assert!(map.set_bit(1, true));
        assert_eq!(map.bit_at(1).unwrap(), true);
        assert!(map.set_bit(8, true));
        assert_eq!(map.bit_at(8).unwrap(), true);
        assert!(map.set_bit(9, true));
        assert_eq!(map.bit_at(9).unwrap(), true);

        assert_eq!(map.count_ones(), 4);
    }

    #[test]
    fn test_count_zeros() {
        let mut map = BitMap::new(1024);

        assert!(map.set_bit(0, true));
        assert_eq!(map.bit_at(0).unwrap(), true);
        assert!(map.set_bit(1, true));
        assert_eq!(map.bit_at(1).unwrap(), true);
        assert!(map.set_bit(8, true));
        assert_eq!(map.bit_at(8).unwrap(), true);
        assert!(map.set_bit(9, true));
        assert_eq!(map.bit_at(9).unwrap(), true);

        assert_eq!(map.count_zeros(), 1020);
    }

    #[test]
    fn test_set_all_true() {
        let mut map = BitMap::new(1024);
        map.set_all(true);

        for bit in map.into_iter() {
            assert!(bit);
        }
    }

    #[test]
    fn test_set_all_false() {
        let mut map = BitMap::new(1024);
        map.set_all(false);

        for bit in map.into_iter() {
            assert!(!bit);
        }
    }

    #[test]
    fn test_find_next_index() {
        let mut map = BitMap::new(1024);

        assert_eq!(map.find_next_0_index().unwrap(), 0);
    }

    #[test]
    fn test_find_next_index_2() {
        let mut map = BitMap::new(1024);

        for i in 0..125 {
            map.set_bit(i, true);
        }

        assert_eq!(map.find_next_0_index().unwrap(), 125);
    }

    #[test]
    fn test_find_next_index_3() {
        let mut map = BitMap::new(1024);

        map.set_all(true);

        assert!(map.find_next_0_index().is_none());
    }
}
