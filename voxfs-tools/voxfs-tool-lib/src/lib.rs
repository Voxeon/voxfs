mod error;
mod handler;
mod manager;

pub use error::MKImageError;
pub use handler::Handler;
pub use manager::Manager;
use byte_unit::Byte;

pub fn sized_string_to_u64(string: &str) -> Option<u64> {
    return match Byte::from_str(string) {
        Ok(b) => Some(b.get_bytes() as u64),
        Err(_) => None,
    };
}

pub fn u64_to_sized_string(n: u64) -> String {
    return Byte::from(n).get_appropriate_unit(false).to_string();
}

#[cfg(test)]
mod tests {
    use super::sized_string_to_u64;

    #[test]
    fn test_no_suffix() {
        assert_eq!(sized_string_to_u64("12").unwrap(), 12)
    }

    #[test]
    fn test_no_suffix_2() {
        assert_eq!(sized_string_to_u64("12333").unwrap(), 12333)
    }

    #[test]
    fn test_kib() {
        assert_eq!(sized_string_to_u64("123KiB").unwrap(), 125_952)
    }

    #[test]
    fn test_mib() {
        assert_eq!(sized_string_to_u64("123MiB").unwrap(), 128_974_848)
    }

    #[test]
    fn test_gib() {
        assert_eq!(sized_string_to_u64("123GiB").unwrap(), 132_070_244_352)
    }

    #[test]
    fn test_fail() {
        assert!(sized_string_to_u64("123AB").is_none())
    }

    #[test]
    fn test_fail_2() {
        assert!(sized_string_to_u64("GB").is_none())
    }

    #[test]
    fn test_fail_3() {
        assert!(sized_string_to_u64("G2B").is_none())
    }

    #[test]
    fn test_fail_4() {
        assert!(sized_string_to_u64("G2GB").is_none())
    }
}
