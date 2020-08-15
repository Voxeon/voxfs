pub fn rightmost_unset_bit(n: u64) -> usize {
    return rightmost_set_bit(!n);
}

pub fn rightmost_set_bit(n: u64) -> usize {
    let mut m = 1;
    let mut index = 0;

    while (m & n) == 0 {
        m <<= 1;
        index += 1;
    }

    return index;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_set_bit_1() {
        let n = 0b01001;
        assert_eq!(rightmost_set_bit(n), 0);
    }

    #[test]
    pub fn test_set_bit_2() {
        let n = 0b01000;
        assert_eq!(rightmost_set_bit(n), 3);
    }

    #[test]
    pub fn test_unset_bit_1() {
        let n = 0b01111;
        assert_eq!(rightmost_unset_bit(n), 4);
    }

    #[test]
    pub fn test_unset_bit_2() {
        let n = 0b01000;
        assert_eq!(rightmost_unset_bit(n), 0);
    }
}
