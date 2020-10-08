use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::{Debug, Display};

macro_rules! enum_variant_stringify {
    ($self:expr, [$($var:ident),+]) => {
        match $self {
            $(
               $var => stringify!($var),
            )+
            _ => "",
        }
    }
}

pub trait VoxFSErrorConvertible: Debug {
    /// If this is an internal error this will succeed otherwise by default it will return None.
    fn into_voxfs_error(self) -> VoxFSError<Self>
    where
        Self: Sized,
    {
        return VoxFSError::DiskError(self);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    FailedToFreeINode,
    TagNotAppliedToINode,
    CorruptedIndirectINode,
    CouldNotFindINode,
    FailedToSetBitmapBit,
    ExpectedIndirectNode,
    InvalidTagName,
    TagExistsWithName(String),
    InvalidFileName,
    MoreNamesThanTagsProvided,
    NoTagsWithNames(Vec<String>),
    DiskError(E),
}

impl<E: Display> core::fmt::Display for VoxFSError<E> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use VoxFSError::*;

        match self {
            DiskError(e) => write!(f, "Disk error: {}", e),
            NoTagsWithNames(names) => {
                let mut names_str = String::new();

                if names.len() > 0 {
                    names_str = names[0].clone();

                    for i in 1..names.len() {
                        names_str.push_str(", ");
                        names_str.push_str(&names[i]);
                    }
                }

                write!(f, "No tags with names: {}", names_str)
            },
            TagExistsWithName(n) => write!(f, "TagExistsWithName({})", n),
            _ => write!(
                f,
                "{}",
                enum_variant_stringify!(
                    self,
                    [
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
                        FailedToFreeINode,
                        TagNotAppliedToINode,
                        CorruptedIndirectINode,
                        CouldNotFindINode,
                        FailedToSetBitmapBit,
                        ExpectedIndirectNode,
                        InvalidTagName,
                        InvalidFileName,
                        MoreNamesThanTagsProvided
                    ]
                )
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::VoxFSError;
    use alloc::string::String;

    #[derive(Debug)]
    struct DummyError;

    impl core::fmt::Display for DummyError {
        fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
            write!(f, "DummyError")
        }
    }

    #[test]
    fn test_fmt_1() {
        let err: VoxFSError<DummyError> = VoxFSError::TagNotAppliedToINode;
        assert_eq!("TagNotAppliedToINode", format!("{}", err));
    }

    #[test]
    fn test_fmt_2() {
        let err: VoxFSError<DummyError> = VoxFSError::InvalidBlockSize;
        assert_eq!("InvalidBlockSize", format!("{}", err));
    }

    #[test]
    fn test_fmt_3() {
        let err: VoxFSError<DummyError> = VoxFSError::DiskError(DummyError);
        assert_eq!("Disk error: DummyError", format!("{}", err));
    }

    #[test]
    fn test_fmt_4() {
        let err: VoxFSError<DummyError> = VoxFSError::NoTagsWithNames(vec![String::from("test")]);
        assert_eq!("No tags with names: test", format!("{}", err));
    }

    #[test]
    fn test_fmt_5() {
        let err: VoxFSError<DummyError> = VoxFSError::NoTagsWithNames(vec![
            String::from("test"),
            String::from("test"),
            String::from("test"),
        ]);
        assert_eq!("No tags with names: test, test, test", format!("{}", err));
    }
}
