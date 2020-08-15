use core::fmt::Debug;

pub trait AidFSErrorConvertible: Debug {
    /// If this is an internal error this will succeed otherwise by default it will return None.
    fn into_aidfs_error(self) -> AidFSError<Self>
    where
        Self: Sized,
    {
        return AidFSError::DiskError(self);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AidFSError<E> {
    InvalidBlockSize,
    NoFreeInode,
    NoFreeTag,
    CorruptedTag,
    CorruptedINode,
    DiskError(E),
}
