extern crate voxfs;
use voxfs::{Disk, INodeFlags, TagFlags};

mod common;
use common::*;

#[test]
fn test_list_all_inodes() {
    let mut handler = Handler::new(4096 * 30); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let file_contents = "The file contents are testing, 1234, ok so this should be one block!"
        .as_bytes()
        .to_vec();

    let mut comp_nodes = Vec::new();

    comp_nodes.push(
        disk.create_new_file(
            "test_file",
            INodeFlags::new(true, true, true, false),
            file_contents.clone(),
        )
        .unwrap(),
    );

    let mut ultra_large_file = vec![0u8; 4096 * 6]; // We want to take up more than 5 blocks so an indirect inode is required.
    ultra_large_file[0] = 0xff;

    comp_nodes.push(
        disk.create_new_file(
            "test_file_2",
            INodeFlags::new(true, true, true, false),
            ultra_large_file,
        )
        .unwrap(),
    );

    let file_contents_2 = "Different file contents for this file!".as_bytes().to_vec();

    comp_nodes.push(
        disk.create_new_file(
            "test_file_3",
            INodeFlags::new(true, true, true, false),
            file_contents_2.clone(),
        )
        .unwrap(),
    );

    let nodes = disk.list_inodes();

    assert_eq!(nodes, comp_nodes);
}

#[test]
fn test_list_all_inodes_30() {
    let mut handler = Handler::new(4096 * 300); // Disk size of 1200 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let file_contents = "The file contents are testing, 1234, ok so this should be one block!"
        .as_bytes()
        .to_vec();

    let mut comp_nodes = Vec::new();

    for i in 0..30 {
        comp_nodes.push(
            disk.create_new_file(
                &format!("test_file_{}", i),
                INodeFlags::new(true, true, true, false),
                file_contents.clone(),
            )
            .unwrap(),
        );
    }

    let nodes = disk.list_inodes();

    assert_eq!(nodes, comp_nodes);
}

#[test]
fn test_list_all_inodes_300() {
    let mut handler = Handler::new(4096 * 1000); // Disk size of 4000 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let file_contents = "The file contents are testing, 1234, ok so this should be one block!"
        .as_bytes()
        .to_vec();

    let mut comp_nodes = Vec::new();

    for i in 0..300 {
        comp_nodes.push(
            disk.create_new_file(
                &format!("test_file_{}", i),
                INodeFlags::new(true, true, true, false),
                file_contents.clone(),
            )
            .unwrap(),
        );
    }

    let nodes = disk.list_inodes();

    assert_eq!(nodes, comp_nodes);
}

#[test]
fn test_list_inodes_with_tags() {
    let mut handler = Handler::new(4096 * 1000); // Disk size of 4000 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let file_contents = "The file contents are testing, 1234, ok so this should be one block!"
        .as_bytes()
        .to_vec();

    let mut comp_nodes = Vec::new();

    let custom_tag_1 = disk
        .create_new_tag("tag_1", TagFlags::new(true, true))
        .unwrap();

    assert_eq!(custom_tag_1.index(), 1);

    let custom_tag_2 = disk
        .create_new_tag("tag_2", TagFlags::new(true, true))
        .unwrap();

    assert_eq!(custom_tag_2.index(), 2);

    for i in 0..300 {
        comp_nodes.push(
            disk.create_new_file(
                &format!("test_file_{}", i),
                INodeFlags::new(true, true, true, false),
                file_contents.clone(),
            )
            .unwrap(),
        );
    }

    for i in 0..100 {
        disk.apply_tag(custom_tag_1.index(), comp_nodes[i].index())
            .unwrap();
    }

    assert_eq!(
        comp_nodes[0..100].to_vec(),
        disk.list_nodes_with_tags(vec![custom_tag_1.index()])
            .unwrap()
    );
    assert_eq!(
        disk.list_nodes_with_tag(custom_tag_1.index()).unwrap(),
        disk.list_nodes_with_tags(vec![custom_tag_1.index()])
            .unwrap()
    );
}

#[test]
fn test_list_inodes_with_tags_2() {
    let mut handler = Handler::new(4096 * 1000); // Disk size of 4000 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let file_contents = "The file contents are testing, 1234, ok so this should be one block!"
        .as_bytes()
        .to_vec();

    let mut comp_nodes = Vec::new();

    let custom_tag_1 = disk
        .create_new_tag("tag_1", TagFlags::new(true, true))
        .unwrap();

    assert_eq!(custom_tag_1.index(), 1);

    let custom_tag_2 = disk
        .create_new_tag("tag_2", TagFlags::new(true, true))
        .unwrap();

    assert_eq!(custom_tag_2.index(), 2);

    for i in 0..300 {
        comp_nodes.push(
            disk.create_new_file(
                &format!("test_file_{}", i),
                INodeFlags::new(true, true, true, false),
                file_contents.clone(),
            )
            .unwrap(),
        );
    }

    for i in 0..100 {
        if i < 20 {
            disk.apply_tag(custom_tag_2.index(), comp_nodes[i].index())
                .unwrap();
        }

        disk.apply_tag(custom_tag_1.index(), comp_nodes[i].index())
            .unwrap();
    }

    assert_eq!(
        comp_nodes[0..100].to_vec(),
        disk.list_nodes_with_tags(vec![custom_tag_1.index()])
            .unwrap()
    );
    assert_eq!(
        comp_nodes[0..20].to_vec(),
        disk.list_nodes_with_tags(vec![custom_tag_2.index()])
            .unwrap()
    );
    assert_eq!(
        comp_nodes[0..20].to_vec(),
        disk.list_nodes_with_tags(vec![custom_tag_1.index(), custom_tag_2.index()])
            .unwrap()
    );
}
