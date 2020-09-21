#[derive(Clone, Debug, PartialEq)]
pub struct VisualiserError {
    message: String,
    is_internal: bool,
}

impl VisualiserError {
    pub fn new(msg: &str) -> Self {
        return Self {
            message: msg.to_string(),
            is_internal: false,
        };
    }

    pub fn new_internal(msg: &str) -> Self {
        return Self {
            message: msg.to_string(),
            is_internal: true,
        };
    }
}

impl std::fmt::Display for VisualiserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_internal {
            return write!(f, "An internal error occurred.");
        } else {
            return write!(f, "{}", self.message);
        }
    }
}
