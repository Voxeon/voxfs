use chrono::DateTime;
use chrono::Utc;
use voxfs::OSManager;

#[derive(Copy, Clone, Debug, PartialOrd, PartialEq, Eq, Ord)]
pub struct Manager {}

impl Manager {
    pub fn new() -> Self {
        return Self {};
    }
}

impl OSManager for Manager {
    fn current_time(&self) -> DateTime<Utc> {
        return Utc::now();
    }
}
