extern crate voxfs;
use voxfs::{Disk, INodeFlags, TagFlags, VoxFSError};

mod common;
use common::*;

#[test]
fn test_tags() {
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
            "test_file",
            INodeFlags::new(true, true, true, false),
            file_contents_2.clone(),
        )
        .unwrap(),
    );

    disk.apply_tag(0, comp_nodes[0].index()).unwrap();
    disk.apply_tag(0, comp_nodes[1].index()).unwrap();
    disk.apply_tag(0, comp_nodes[2].index()).unwrap();

    assert_eq!(disk.list_nodes_with_tag(0).unwrap(), comp_nodes);
}

#[test]
fn test_tags_12() {
    let mut handler = Handler::new(4096 * 30); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let file_contents = "The file contents are testing, 1234, ok so this should be one block!"
        .as_bytes()
        .to_vec();

    let mut comp_nodes = Vec::new();

    for i in 0..12 {
        comp_nodes.push(
            disk.create_new_file(
                "test_file",
                INodeFlags::new(true, true, true, false),
                file_contents.clone(),
            )
            .unwrap(),
        );

        disk.apply_tag(0, comp_nodes[i].index()).unwrap();
    }

    assert_eq!(disk.list_nodes_with_tag(0).unwrap(), comp_nodes);
}

#[test]
fn test_tags_duplicate() {
    let mut handler = Handler::new(4096 * 30); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let file_contents = "The file contents are testing, 1234, ok so this should be one block!"
        .as_bytes()
        .to_vec();

    let mut comp_nodes = Vec::new();

    for i in 0..11 {
        comp_nodes.push(
            disk.create_new_file(
                "test_file",
                INodeFlags::new(true, true, true, false),
                file_contents.clone(),
            )
            .unwrap(),
        );

        disk.apply_tag(0, comp_nodes[i].index()).unwrap();
    }

    assert_eq!(disk.list_nodes_with_tag(0).unwrap(), comp_nodes);

    assert_eq!(
        disk.apply_tag(0, comp_nodes[0].index()).unwrap_err(),
        VoxFSError::<common::Error>::TagAlreadyAppliedToINode
    );
}

#[test]
fn test_tags_indirect() {
    let mut handler = Handler::new(4096 * 30); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let file_contents = "The file contents are testing, 1234, ok so this should be one block!"
        .as_bytes()
        .to_vec();

    let mut comp_nodes = Vec::new();

    for i in 0..21 {
        comp_nodes.push(
            disk.create_new_file(
                "test_file",
                INodeFlags::new(true, true, true, false),
                file_contents.clone(),
            )
            .unwrap(),
        );

        disk.apply_tag(0, comp_nodes[i].index()).unwrap();
    }

    assert_eq!(disk.list_nodes_with_tag(0).unwrap(), comp_nodes);
}

#[test]
fn test_tags_indirects() {
    let mut handler = Handler::new(4096 * 1000); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let file_contents = "The file contents are testing, 1234, ok so this should be one block!"
        .as_bytes()
        .to_vec();

    let mut comp_nodes = Vec::new();

    for i in 0..730 {
        comp_nodes.push(
            disk.create_new_file(
                "test_file",
                INodeFlags::new(true, true, true, false),
                file_contents.clone(),
            )
            .unwrap(),
        );

        disk.apply_tag(0, comp_nodes[i].index()).unwrap();
    }

    assert_eq!(disk.list_nodes_with_tag(0).unwrap(), comp_nodes);
}

#[test]
fn test_custom_tag() {
    let mut handler = Handler::new(4096 * 30); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let custom_tag = disk
        .create_new_tag("file_1", TagFlags::new(true, true))
        .unwrap();

    assert_eq!(custom_tag.index(), 1);

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
            "test_file",
            INodeFlags::new(true, true, true, false),
            file_contents_2.clone(),
        )
        .unwrap(),
    );

    disk.apply_tag(custom_tag.index(), comp_nodes[0].index())
        .unwrap();
    disk.apply_tag(custom_tag.index(), comp_nodes[1].index())
        .unwrap();
    disk.apply_tag(custom_tag.index(), comp_nodes[2].index())
        .unwrap();

    assert_eq!(
        disk.list_nodes_with_tag(custom_tag.index()).unwrap(),
        comp_nodes
    );
}

#[test]
fn test_remove_tag() {
    let mut handler = Handler::new(4096 * 30); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let custom_tag = disk
        .create_new_tag("file_1", TagFlags::new(true, true))
        .unwrap();

    assert_eq!(custom_tag.index(), 1);

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
            "test_file",
            INodeFlags::new(true, true, true, false),
            file_contents_2.clone(),
        )
        .unwrap(),
    );

    disk.apply_tag(custom_tag.index(), comp_nodes[0].index())
        .unwrap();
    disk.apply_tag(custom_tag.index(), comp_nodes[1].index())
        .unwrap();
    disk.apply_tag(custom_tag.index(), comp_nodes[2].index())
        .unwrap();

    disk.remove_tag_from_inode(1, comp_nodes[0].index())
        .unwrap();

    assert_eq!(disk.list_tags().len(), 2);
    let inodes = disk.list_nodes_with_tag(1).unwrap();
    assert_eq!(inodes[0], comp_nodes[1]);
    assert_eq!(inodes[1], comp_nodes[2]);
}

#[test]
fn test_remove_from_indirect_tag() {
    let mut handler = Handler::new(4096 * 60); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let custom_tag = disk
        .create_new_tag("file_1", TagFlags::new(true, true))
        .unwrap();

    assert_eq!(custom_tag.index(), 1);

    let file_contents = "The file contents are testing, 1234, ok so this should be one block!"
        .as_bytes()
        .to_vec();

    let mut comp_nodes = Vec::new();

    for i in 0..13 {
        comp_nodes.push(
            disk.create_new_file(
                "test_file",
                INodeFlags::new(true, true, true, false),
                file_contents.clone(),
            )
            .unwrap(),
        );

        disk.apply_tag(custom_tag.index(), comp_nodes[i].index())
            .unwrap();
    }

    disk.remove_tag_from_inode(custom_tag.index(), comp_nodes.remove(0).index())
        .unwrap();
    disk.remove_tag_from_inode(custom_tag.index(), comp_nodes.pop().unwrap().index())
        .unwrap();

    assert_eq!(
        comp_nodes,
        disk.list_nodes_with_tag(custom_tag.index()).unwrap()
    );
}

#[test]
fn test_remove_from_large_tag() {
    let mut handler = Handler::new(4096 * 1000); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let custom_tag = disk
        .create_new_tag("file_1", TagFlags::new(true, true))
        .unwrap();

    assert_eq!(custom_tag.index(), 1);

    let file_contents = "The file contents are testing, 1234, ok so this should be one block!"
        .as_bytes()
        .to_vec();

    let mut comp_nodes = Vec::new();

    for i in 0..523 {
        comp_nodes.push(
            disk.create_new_file(
                "test_file",
                INodeFlags::new(true, true, true, false),
                file_contents.clone(),
            )
            .unwrap(),
        );

        disk.apply_tag(custom_tag.index(), comp_nodes[i].index())
            .unwrap();
    }

    assert_eq!(disk.available_data_blocks(), 349);

    // We will remove all the middle block.
    for _ in 0..509 {
        disk.remove_tag_from_inode(custom_tag.index(), comp_nodes.remove(12).index())
            .unwrap();
    }

    assert_eq!(disk.available_data_blocks(), 350);
    assert_eq!(
        comp_nodes,
        disk.list_nodes_with_tag(custom_tag.index()).unwrap()
    );
}

#[test]
fn test_delete_tag() {
    let mut handler = Handler::new(4096 * 30); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let custom_tag = disk
        .create_new_tag("file_1", TagFlags::new(true, true))
        .unwrap();

    assert_eq!(custom_tag.index(), 1);

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
            "test_file",
            INodeFlags::new(true, true, true, false),
            file_contents_2.clone(),
        )
        .unwrap(),
    );

    disk.apply_tag(custom_tag.index(), comp_nodes[0].index())
        .unwrap();
    disk.apply_tag(custom_tag.index(), comp_nodes[1].index())
        .unwrap();
    disk.apply_tag(custom_tag.index(), comp_nodes[2].index())
        .unwrap();

    disk.delete_tag(custom_tag.index()).unwrap();

    assert_eq!(disk.list_tags().len(), 1);

    let custom_tag_2 = disk
        .create_new_tag("file_2", TagFlags::new(true, true))
        .unwrap(); // Create a new tag

    assert_eq!(custom_tag_2.index(), 1);
}

#[test]
fn test_delete_large_tag() {
    let mut handler = Handler::new(4096 * 60); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let custom_tag = disk
        .create_new_tag("file_1", TagFlags::new(true, true))
        .unwrap();

    assert_eq!(custom_tag.index(), 1);

    let file_contents = "The file contents are testing, 1234, ok so this should be one block!"
        .as_bytes()
        .to_vec();

    let mut comp_nodes = Vec::new();

    for i in 0..30 {
        comp_nodes.push(
            disk.create_new_file(
                "test_file",
                INodeFlags::new(true, true, true, false),
                file_contents.clone(),
            )
            .unwrap(),
        );

        disk.apply_tag(custom_tag.index(), comp_nodes[i].index())
            .unwrap();
    }

    disk.delete_tag(custom_tag.index()).unwrap();

    assert_eq!(disk.list_tags().len(), 1);

    let custom_tag_2 = disk
        .create_new_tag("file_2", TagFlags::new(true, true))
        .unwrap(); // Create a new tag

    assert_eq!(custom_tag_2.index(), 1);
}

#[test]
fn test_tag_with_names_1() {
    let mut handler = Handler::new(4096 * 30); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let custom_tag = disk
        .create_new_tag("tag_1", TagFlags::new(true, true))
        .unwrap();

    assert_eq!(custom_tag.index(), 1);

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

    assert_eq!(
        disk.tags_with_names(vec!["tag_1".to_string()]).unwrap(),
        vec![1]
    );
}

#[test]
fn test_tag_with_names_2() {
    let mut handler = Handler::new(4096 * 30); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    let custom_tag = disk
        .create_new_tag("tag_1", TagFlags::new(true, true))
        .unwrap();

    assert_eq!(custom_tag.index(), 1);

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

    assert_eq!(
        disk.tags_with_names(vec!["tag_1".to_string(), "root".to_string()])
            .unwrap(),
        vec![0, 1]
    );
}

#[test]
fn test_tag_with_names_3() {
    let mut handler = Handler::new(4096 * 30); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    for i in 1..10 {
        disk.create_new_tag(&format!("tag_{}", i), TagFlags::new(true, true))
            .unwrap();
    }

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

    assert_eq!(
        disk.tags_with_names(vec![
            "tag_5".to_string(),
            "tag_1".to_string(),
            "tag_3".to_string()
        ])
        .unwrap(),
        vec![1, 3, 5]
    );
}

#[test]
fn test_tag_with_names_fail() {
    let mut handler = Handler::new(4096 * 30); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    for i in 1..10 {
        disk.create_new_tag(&format!("tag_{}", i), TagFlags::new(true, true))
            .unwrap();
    }

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

    assert!(disk
        .tags_with_names(vec![
            "tag_x".to_string(),
            "tag_1".to_string(),
            "tag_3".to_string()
        ])
        .is_err());
}

#[test]
fn test_tag_with_names_fail_2() {
    let mut handler = Handler::new(4096 * 30); // Disk size of 120 KiB
    let mut manager = Manager::new();

    let mut disk = Disk::make_new_filesystem(&mut handler, &mut manager).unwrap();

    for i in 1..10 {
        disk.create_new_tag(&format!("tag_{}", i), TagFlags::new(true, true))
            .unwrap();
    }

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

    assert!(disk
        .tags_with_names(vec![
            "tag_2".to_string(),
            "root0".to_string(),
            "tag_3".to_string()
        ])
        .is_err());
}
