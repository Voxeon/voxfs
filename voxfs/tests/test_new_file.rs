extern crate voxfs;
use voxfs::{ByteSerializable, Disk, DiskHandler, OSManager, TagBlock, TagFlags};

mod common;
use common::*;

#[test]
fn test_create_new_disk() {
    let mut handler = Handler::new(4096 * 5); // Disk size of 16 KiB
    let mut manager = Manager::new();

    let root_tag = TagBlock::new(
        0,
        "root",
        TagFlags::new(true, true),
        manager.current_time().timestamp_nanos() as u64,
        0x0,
        0x0,
        [0u64; 12],
    );

    let mut disk =
        Disk::make_new_filesystem_with_root(&mut handler, &mut manager, root_tag.clone()).unwrap();

    let tags = disk.list_tags();
}
