use std::fmt::Formatter;
use voxfs::VoxFSErrorConvertible;

#[derive(Debug, PartialEq, Clone)]
pub struct MKImageError {
    message: String,
}

impl MKImageError {
    pub fn new(message: &str) -> Self {
        return MKImageError {
            message: String::from(message),
        };
    }

    pub fn get_message(&self) -> String {
        return self.message.clone();
    }
}

impl VoxFSErrorConvertible for MKImageError {}

impl std::fmt::Display for MKImageError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{:?}", self);
    }
}
