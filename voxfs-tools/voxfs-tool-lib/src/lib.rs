mod error;
mod handler;
mod manager;

pub use handler::Handler;
pub use manager::Manager;

pub fn sized_string_to_u64(mut string: String) -> Option<u64> {
    if string.len() <= 2 {
        return match string.parse::<u64>() {
            Ok(i) => Some(i),
            Err(_) => None,
        };
    }
    let last = string.pop()?;

    if last.is_numeric() {
        string.push(last);
        return match string.parse::<u64>() {
            Ok(i) => Some(i),
            Err(_) => None,
        };
    }

    let second_last = string.pop()?;

    if last != 'B' {
        return None;
    }

    let multiplier;

    if second_last == 'K' {
        multiplier = 1024;
    } else if second_last == 'M' {
        multiplier = 1024 * 1024;
    } else if second_last == 'G' {
        multiplier = 1024 * 1024 * 1024;
    } else {
        return None;
    }

    return match string.parse::<u64>() {
        Ok(i) => Some(i * multiplier),
        Err(_) => None,
    };
}

#[cfg(test)]
mod tests {
    use super::sized_string_to_u64;

    #[test]
    fn test_no_suffix() {
        let str = "12".to_string();
        assert_eq!(sized_string_to_u64(str).unwrap(), 12)
    }

    #[test]
    fn test_no_suffix_2() {
        let str = "12333".to_string();
        assert_eq!(sized_string_to_u64(str).unwrap(), 12333)
    }

    #[test]
    fn test_kib() {
        let str = "123KB".to_string();
        assert_eq!(sized_string_to_u64(str).unwrap(), 125_952)
    }

    #[test]
    fn test_mib() {
        let str = "123MB".to_string();
        assert_eq!(sized_string_to_u64(str).unwrap(), 128_974_848)
    }

    #[test]
    fn test_gib() {
        let str = "123GB".to_string();
        assert_eq!(sized_string_to_u64(str).unwrap(), 132_070_244_352)
    }

    #[test]
    fn test_fail() {
        let str = "123AB".to_string();
        assert!(sized_string_to_u64(str).is_none())
    }

    #[test]
    fn test_fail_2() {
        let str = "GB".to_string();
        assert!(sized_string_to_u64(str).is_none())
    }

    #[test]
    fn test_fail_3() {
        let str = "G2B".to_string();
        assert!(sized_string_to_u64(str).is_none())
    }

    #[test]
    fn test_fail_4() {
        let str = "G2GB".to_string();
        assert!(sized_string_to_u64(str).is_none())
    }
}
