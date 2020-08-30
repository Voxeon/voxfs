extern crate voxfs;
use voxfs::{Disk, INodeFlags, OSManager, TagBlock, TagFlags};

mod common;
use common::*;

#[test]
fn test_open_single_file() {
    let mut handler = Handler::new(4096 * 30); // Disk size of 120 KiB
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

    let mut disk =
        Disk::make_new_filesystem_with_root(&mut handler, &mut manager, root_tag.clone()).unwrap();

    let file_contents = "The file contents are testing, 1234, ok so this should be one block!"
        .as_bytes()
        .to_vec();

    let comp_inode = disk
        .create_new_file(
            "test_file",
            INodeFlags::new(true, true, true, false),
            file_contents.clone(),
        )
        .unwrap();

    drop(disk);

    let disk = Disk::open_disk(&mut handler, &mut manager).unwrap();
    let tags = disk.list_tags();
    let inodes = disk.list_inodes();

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0], root_tag);
    assert_eq!(inodes.len(), 1);
    assert_eq!(inodes[0], comp_inode);

    assert_eq!(file_contents, disk.read_file(&comp_inode).unwrap());
}

#[test]
fn test_open_single_large_file() {
    let mut handler = Handler::new(4096 * 30); // Disk size of 120 KiB
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

    let mut disk =
        Disk::make_new_filesystem_with_root(&mut handler, &mut manager, root_tag.clone()).unwrap();

    let file_contents = {
        let mut res = Vec::new();

        for i in 0..12042 {
            res.push((i % 256) as u8);
        }

        res
    };

    let comp_inode = disk
        .create_new_file(
            "test_file",
            INodeFlags::new(true, true, true, false),
            file_contents.clone(),
        )
        .unwrap();

    drop(disk);

    let disk = Disk::open_disk(&mut handler, &mut manager).unwrap();
    let tags = disk.list_tags();
    let inodes = disk.list_inodes();

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0], root_tag);
    assert_eq!(inodes.len(), 1);
    assert_eq!(inodes[0], comp_inode);

    assert_eq!(file_contents, disk.read_file(&comp_inode).unwrap());
}

#[test]
fn test_open_multiple_small_files() {
    let mut handler = Handler::new(4096 * 300); // Disk size of 120 KiB
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

    let mut disk =
        Disk::make_new_filesystem_with_root(&mut handler, &mut manager, root_tag.clone()).unwrap();

    let file_contents = "The file contents are testing, 1234, ok so this should be one block!"
        .as_bytes()
        .to_vec();

    let mut comp_inodes = Vec::new();

    for i in 0..30 {
        comp_inodes.push(
            disk.create_new_file(
                &format!("test_file_{}", i),
                INodeFlags::new(true, true, true, false),
                file_contents.clone(),
            )
            .unwrap(),
        );
    }

    drop(disk);

    let disk = Disk::open_disk(&mut handler, &mut manager).unwrap();
    let tags = disk.list_tags();
    let inodes = disk.list_inodes();

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0], root_tag);
    assert_eq!(inodes.len(), comp_inodes.len());
    assert_eq!(inodes, comp_inodes);

    for inode in inodes {
        assert_eq!(file_contents, disk.read_file(&inode).unwrap());
    }
}

#[test]
fn test_open_multiple_large_files() {
    let mut handler = Handler::new(4096 * 300); // Disk size of 120 KiB
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

    let mut disk =
        Disk::make_new_filesystem_with_root(&mut handler, &mut manager, root_tag.clone()).unwrap();

    let file_contents = {
        let mut res = Vec::new();

        for i in 0..1200 {
            res.push((i % 256) as u8);
        }

        res
    };

    let mut comp_inodes = Vec::new();

    for i in 0..6 {
        comp_inodes.push(
            disk.create_new_file(
                &format!("test_file_{}", i),
                INodeFlags::new(true, true, true, false),
                file_contents.clone(),
            )
            .unwrap(),
        );
    }

    drop(disk);

    let disk = Disk::open_disk(&mut handler, &mut manager).unwrap();
    let tags = disk.list_tags();
    let inodes = disk.list_inodes();

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0], root_tag);
    assert_eq!(inodes.len(), comp_inodes.len());
    assert_eq!(inodes, comp_inodes);

    for inode in inodes {
        assert_eq!(file_contents, disk.read_file(&inode).unwrap());
    }
}

#[test]
fn test_open_multiple_small_files_tagged() {
    let mut handler = Handler::new(4096 * 300); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let file_contents = "The file contents are testing, 1234, ok so this should be one block!"
        .as_bytes()
        .to_vec();

    let mut comp_inodes = Vec::new();

    for i in 0..30 {
        comp_inodes.push(
            disk.create_new_file(
                &format!("test_file_{}", i),
                INodeFlags::new(true, true, true, false),
                file_contents.clone(),
            )
            .unwrap(),
        );

        disk.apply_tag(0, &comp_inodes[i]).unwrap();
    }

    drop(disk);

    let disk = Disk::open_disk(&mut handler, &mut manager).unwrap();
    let tags = disk.list_tags();
    let inodes = disk.list_tag_nodes(0).unwrap();

    assert_eq!(tags.len(), 1);
    assert_eq!(inodes.len(), comp_inodes.len());
    assert_eq!(inodes, comp_inodes);

    for inode in inodes {
        assert_eq!(file_contents, disk.read_file(&inode).unwrap());
    }
}

#[test]
fn test_open_multiple_small_files_tag_removed() {
    let mut handler = Handler::new(4096 * 1000); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let file_contents = "The file contents are testing, 1234, ok so this should be one block!"
        .as_bytes()
        .to_vec();

    let mut comp_inodes = Vec::new();

    for i in 0..523 {
        comp_inodes.push(
            disk.create_new_file(
                &format!("test_file_{}", i),
                INodeFlags::new(true, true, true, false),
                file_contents.clone(),
            )
            .unwrap(),
        );

        disk.apply_tag(0, &comp_inodes[i]).unwrap();
    }

    for _ in 0..510 {
        disk.remove_tag_from_inode(0, &comp_inodes.remove(12))
            .unwrap();
    }

    drop(disk);

    let disk = Disk::open_disk(&mut handler, &mut manager).unwrap();
    let tags = disk.list_tags();
    let inodes = disk.list_tag_nodes(0).unwrap();

    assert_eq!(tags.len(), 1);
    assert_eq!(inodes.len(), comp_inodes.len());
    assert_eq!(inodes, comp_inodes);

    for inode in inodes {
        assert_eq!(file_contents, disk.read_file(&inode).unwrap());
    }
}
