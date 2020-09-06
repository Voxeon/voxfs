extern crate voxfs;
use voxfs::{Disk, INodeFlags};

mod common;
use common::*;

#[test]
fn test_append_no_new_block() {
    let mut handler = Handler::new(4096 * 30); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let mut file_contents = "The file contents are testing, 1234, ok so this should be one "
        .as_bytes()
        .to_vec();

    let node_index = disk
        .create_new_file(
            "test_file",
            INodeFlags::new(true, true, true, false),
            file_contents.clone(),
        )
        .unwrap()
        .index();

    let b = b"block!";
    file_contents.extend_from_slice(b);

    disk.append_file_bytes(node_index, &b.to_vec()).unwrap();

    assert_eq!(
        handler.dump_disk()[32768..32768 + file_contents.len()].to_vec(),
        file_contents
    );
}

#[test]
fn test_append_new_block() {
    let mut handler = Handler::new(4096 * 30); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let mut file_contents = "The file contents are testing, 1234, ok so this should be one block!"
        .as_bytes()
        .to_vec();

    let node_index = disk
        .create_new_file(
            "test_file",
            INodeFlags::new(true, true, true, false),
            file_contents.clone(),
        )
        .unwrap()
        .index();

    let b = vec![13u8; 4097];
    file_contents.append(&mut b.clone());

    disk.append_file_bytes(node_index, &b).unwrap();

    assert_eq!(disk.read_file(node_index).unwrap(), file_contents);

    assert_eq!(
        handler.dump_disk()[32768..32768 + file_contents.len()].to_vec(),
        file_contents
    );
}

#[test]
fn test_append_new_block_indirect_required() {
    let mut handler = Handler::new(4096 * 50); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let mut file_contents = "The file contents are testing, 1234, ok so this should be one block!"
        .as_bytes()
        .to_vec();

    let node_index = disk
        .create_new_file(
            "test_file",
            INodeFlags::new(true, true, true, false),
            file_contents.clone(),
        )
        .unwrap()
        .index();

    for _ in 0..16 {
        let mut b = Vec::new();

        for i in 0..4097 {
            b.push((i % 256) as u8);
        }

        file_contents.append(&mut b.clone());

        disk.append_file_bytes(node_index, &b).unwrap();
    }

    assert_eq!(disk.read_file(node_index).unwrap(), file_contents);
}
