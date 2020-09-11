use voxfs::VoxFSErrorConvertible;

#[derive(Debug, PartialEq, Clone)]
pub struct MKImageError {
    message: String,
}

impl MKImageError {
    pub fn new(message: &str) -> Self {
        return MKImageError {message: String::from(message)};
    }

    pub fn get_message(&self) -> String {
        return self.message.clone();
    }
}

impl VoxFSErrorConvertible for MKImageError {}