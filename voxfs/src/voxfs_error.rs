use core::fmt::Debug;

pub trait VoxFSErrorConvertible: Debug {
    /// If this is an internal error this will succeed otherwise by default it will return None.
    fn into_voxfs_error(self) -> VoxFSError<Self>
    where
        Self: Sized,
    {
        return VoxFSError::DiskError(self);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoxFSError<E> {
    InvalidBlockSize,
    NoFreeInode,
    NoFreeTag,
    CorruptedTag,
    CorruptedIndirectTag,
    CorruptedINode,
    CorruptedSuperBlock,
    NotEnoughFreeDataBlocks,
    BlockAlreadyAllocated,
    CouldNotFindTag,
    TagAlreadyAppliedToINode,
    FailedToAppendToNewTag,
    InternalIndexLocationError,
    FailedIndirectTagAppend,
    FailedToFreeTag,
    FailedToFreeBlock,
    TagNotAppliedToINode,
    CorruptedIndirectINode,
    DiskError(E),
}
