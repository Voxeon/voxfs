extern crate voxfs;
use voxfs::{Disk, INodeFlags};

mod common;
use common::*;

#[test]
fn test_read_small_file() {
    let mut handler = Handler::new(4096 * 30); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let file_contents =
        b"The file contents are testing, 1234, ok so this should be one block!".to_vec();

    let node = disk
        .create_new_file(
            "test_file",
            INodeFlags::new(true, true, true, false),
            file_contents.clone(),
        )
        .unwrap();

    let read_contents = disk.read_file(node.index()).unwrap();

    assert_eq!(read_contents[..file_contents.len()].to_vec(), file_contents);
}

#[test]
fn test_read_large_file() {
    let mut handler = Handler::new(4096 * 30); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let mut file_contents = Vec::new();

    for i in 0..32_768 {
        file_contents.push((i % 256) as u8);
    }

    let node = disk
        .create_new_file(
            "test_file",
            INodeFlags::new(true, true, true, false),
            file_contents.clone(),
        )
        .unwrap();

    let read_contents = disk.read_file(node.index()).unwrap();

    assert_eq!(read_contents[..file_contents.len()].to_vec(), file_contents);
}
