extern crate voxfs;
use voxfs::{Disk, INodeFlags, TagFlags};

mod common;
use common::*;

#[test]
fn test_delete_file() {
    let mut handler = Handler::new(4096 * 30); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let file_contents = "The file contents are testing, 1234, ok so this should be one block!"
        .as_bytes()
        .to_vec();

    let available_blocks = disk.available_data_blocks();

    disk.create_new_file(
        "test_file",
        INodeFlags::new(true, true, true, false),
        file_contents.clone(),
    )
    .unwrap();

    let index = disk.list_inodes()[0].index();
    disk.delete_file(index).unwrap();

    assert_eq!(disk.list_inodes().len(), 0);
    assert!(disk.read_file(index).is_err());
    assert_eq!(disk.available_data_blocks(), available_blocks);
}

#[test]
fn test_delete_huge_file() {
    let mut handler = Handler::new(4096 * 30); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let file_contents = {
        let mut res = Vec::new();

        for i in 0..49152 {
            res.push((i % 256) as u8);
        }

        res
    };

    let available_blocks = disk.available_data_blocks();

    disk.create_new_file(
        "test_file",
        INodeFlags::new(true, true, true, false),
        file_contents.clone(),
    )
        .unwrap().index();

    let index = disk.list_inodes()[0].index();
    disk.delete_file(index).unwrap();

    assert_eq!(disk.list_inodes().len(), 0);
    assert!(disk.read_file(index).is_err());
    assert_eq!(disk.available_data_blocks(), available_blocks);
}

#[test]
fn test_delete_huge_file_appends() {
    let mut handler = Handler::new(4096 * 30); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let file_contents = {
        let mut res = Vec::new();

        for i in 0..4097 {
            res.push((i % 256) as u8);
        }

        res
    };

    let available_blocks = disk.available_data_blocks();

    disk.create_new_file(
        "test_file",
        INodeFlags::new(true, true, true, false),
        file_contents.clone(),
    )
        .unwrap().index();

    let index = disk.list_inodes()[0].index();

    for _ in 0..10 {
        disk.append_file_bytes(index, &file_contents).unwrap();
    }

    disk.delete_file(index).unwrap();

    assert_eq!(disk.list_inodes().len(), 0);
    assert!(disk.read_file(index).is_err());
    assert_eq!(disk.available_data_blocks(), available_blocks);
}

#[test]
fn test_delete_fail() {
    let mut handler = Handler::new(4096 * 30); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    assert!(disk.delete_file(0).is_err());
}

#[test]
fn test_delete_file_tags() {
    let mut handler = Handler::new(4096 * 30); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let file_contents = "The file contents are testing, 1234, ok so this should be one block!"
        .as_bytes()
        .to_vec();


    disk.create_new_file(
        "test_file",
        INodeFlags::new(true, true, true, false),
        file_contents.clone(),
    )
        .unwrap();

    disk.create_new_file(
        "test_file2",
        INodeFlags::new(true, true, true, false),
        file_contents.clone(),
    )
        .unwrap();

    disk.create_new_file(
        "test_file3",
        INodeFlags::new(true, true, true, false),
        file_contents.clone(),
    )
        .unwrap();

    let tag_index = disk.create_new_tag("test", TagFlags::new(true, true)).unwrap().index();
    disk.apply_tag(tag_index,disk.list_inodes()[0].index()).unwrap();
    disk.apply_tag(tag_index,disk.list_inodes()[1].index()).unwrap();
    disk.apply_tag(tag_index,disk.list_inodes()[2].index()).unwrap();

    let index = disk.list_inodes()[1].index();
    assert_eq!(disk.list_tag_nodes(tag_index).unwrap().len(), 3);
    disk.delete_file(index).unwrap();
    assert_eq!(disk.list_tag_nodes(tag_index).unwrap().len(), 2);
}
