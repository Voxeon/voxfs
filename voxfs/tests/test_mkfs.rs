extern crate voxfs;
use voxfs::{ByteSerializable, Disk, DiskHandler, OSManager, TagBlock, TagFlags};

mod common;
use common::*;

#[test]
fn test_create_new_disk() {
    let mut handler = Handler::new(4096 * 400); // Disk size of 1600 KiB
    let mut manager = Manager::new();

    let root_tag = TagBlock::new(
        0,
        "root",
        TagFlags::new(true, true),
        manager.current_time(),
        0x0,
        0x0,
        [0u64; 12],
    );

    let disk =
        Disk::make_new_filesystem_with_root(&mut handler, &mut manager, root_tag.clone()).unwrap();

    let tags = disk.list_tags();

    let mut tag_bitmap_bits = vec![0u8; 4096];
    tag_bitmap_bits[0] = 0b1;

    assert_eq!(tag_bitmap_bits, handler.read_bytes(4096, 4096).unwrap());
    assert_eq!(vec![0u8; 8192], handler.read_bytes(8192, 8192).unwrap()); // Ensure that data blocks and inode maps are 0

    assert_eq!(
        root_tag.to_bytes().to_vec(),
        handler.read_bytes(0x4000, TagBlock::size()).unwrap()
    );

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0], root_tag);
}
