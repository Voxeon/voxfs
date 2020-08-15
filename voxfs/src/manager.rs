use chrono::DateTime;
use chrono::Utc;
use core::fmt::Debug;

/// Provide OS specific methods
pub trait OSManager: Debug {
    fn current_time(&self) -> DateTime<Utc>;
}
