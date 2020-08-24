extern crate voxfs;
use voxfs::{Disk, INodeFlags, OSManager, TagBlock, TagFlags};

mod common;
use common::*;

#[test]
fn test_create_new_file() {
    let mut handler = Handler::new(4096 * 30); // Disk size of 120 KiB
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

    let file_contents = "The file contents are testing, 1234, ok so this should be one block!"
        .as_bytes()
        .to_vec();

    disk.create_new_file_first_free(
        "test_file",
        INodeFlags::new(true, true, true, false),
        file_contents.clone(),
    )
    .unwrap();

    let mut ultra_large_file = vec![0u8; 4096 * 6]; // We want to take up more than 5 blocks so an indirect inode is required.
    ultra_large_file[0] = 0xff;

    disk.create_new_file_first_free(
        "test_file_2",
        INodeFlags::new(true, true, true, false),
        ultra_large_file,
    )
    .unwrap();

    assert_eq!(
        handler.dump_disk()[32768..32768 + file_contents.len()].to_vec(),
        file_contents
    );
}
