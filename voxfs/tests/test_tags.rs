extern crate voxfs;
use voxfs::{Disk, INodeFlags, OSManager, TagBlock, TagFlags, VoxFSError};

mod common;
use common::*;

#[test]
fn test_tags() {
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

    let mut comp_nodes = Vec::new();

    comp_nodes.push(
        disk.create_new_file_first_free(
            "test_file",
            INodeFlags::new(true, true, true, false),
            file_contents.clone(),
        )
        .unwrap(),
    );

    let mut ultra_large_file = vec![0u8; 4096 * 6]; // We want to take up more than 5 blocks so an indirect inode is required.
    ultra_large_file[0] = 0xff;

    comp_nodes.push(
        disk.create_new_file_first_free(
            "test_file_2",
            INodeFlags::new(true, true, true, false),
            ultra_large_file,
        )
        .unwrap(),
    );

    let file_contents_2 = "Different file contents for this file!".as_bytes().to_vec();

    comp_nodes.push(
        disk.create_new_file_first_free(
            "test_file",
            INodeFlags::new(true, true, true, false),
            file_contents_2.clone(),
        )
        .unwrap(),
    );

    disk.apply_tag(0, &comp_nodes[0]).unwrap();
    disk.apply_tag(0, &comp_nodes[1]).unwrap();
    disk.apply_tag(0, &comp_nodes[2]).unwrap();

    assert_eq!(disk.list_tag_nodes(0).unwrap(), comp_nodes);
}

#[test]
fn test_tags_12() {
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

    let mut comp_nodes = Vec::new();

    for i in 0..12 {
        comp_nodes.push(
            disk.create_new_file_first_free(
                "test_file",
                INodeFlags::new(true, true, true, false),
                file_contents.clone(),
            )
            .unwrap(),
        );

        disk.apply_tag(0, &comp_nodes[i]).unwrap();
    }

    assert_eq!(disk.list_tag_nodes(0).unwrap(), comp_nodes);
}

#[test]
fn test_tags_duplicate() {
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

    let mut comp_nodes = Vec::new();

    for i in 0..11 {
        comp_nodes.push(
            disk.create_new_file_first_free(
                "test_file",
                INodeFlags::new(true, true, true, false),
                file_contents.clone(),
            )
            .unwrap(),
        );

        disk.apply_tag(0, &comp_nodes[i]).unwrap();
    }

    assert_eq!(disk.list_tag_nodes(0).unwrap(), comp_nodes);

    assert_eq!(
        disk.apply_tag(0, &comp_nodes[0]).unwrap_err(),
        VoxFSError::<common::Error>::TagAlreadyAppliedToINode
    );
}

#[test]
fn test_tags_indirect() {
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

    let mut comp_nodes = Vec::new();

    for i in 0..21 {
        comp_nodes.push(
            disk.create_new_file_first_free(
                "test_file",
                INodeFlags::new(true, true, true, false),
                file_contents.clone(),
            )
            .unwrap(),
        );

        disk.apply_tag(0, &comp_nodes[i]).unwrap();
    }

    assert_eq!(disk.list_tag_nodes(0).unwrap(), comp_nodes);
}

#[test]
fn test_tags_indirects() {
    let mut handler = Handler::new(4096 * 1000); // Disk size of 120 KiB
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

    let mut comp_nodes = Vec::new();

    for i in 0..730 {
        comp_nodes.push(
            disk.create_new_file_first_free(
                "test_file",
                INodeFlags::new(true, true, true, false),
                file_contents.clone(),
            )
            .unwrap(),
        );

        disk.apply_tag(0, &comp_nodes[i]).unwrap();
    }

    assert_eq!(disk.list_tag_nodes(0).unwrap(), comp_nodes);
}

#[test]
fn test_custom_tag() {
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

    let custom_tag = disk
        .create_new_tag("file_1", TagFlags::new(true, true))
        .unwrap();

    assert_eq!(custom_tag.index(), 1);

    let file_contents = "The file contents are testing, 1234, ok so this should be one block!"
        .as_bytes()
        .to_vec();

    let mut comp_nodes = Vec::new();

    comp_nodes.push(
        disk.create_new_file_first_free(
            "test_file",
            INodeFlags::new(true, true, true, false),
            file_contents.clone(),
        )
        .unwrap(),
    );

    let mut ultra_large_file = vec![0u8; 4096 * 6]; // We want to take up more than 5 blocks so an indirect inode is required.
    ultra_large_file[0] = 0xff;

    comp_nodes.push(
        disk.create_new_file_first_free(
            "test_file_2",
            INodeFlags::new(true, true, true, false),
            ultra_large_file,
        )
        .unwrap(),
    );

    let file_contents_2 = "Different file contents for this file!".as_bytes().to_vec();

    comp_nodes.push(
        disk.create_new_file_first_free(
            "test_file",
            INodeFlags::new(true, true, true, false),
            file_contents_2.clone(),
        )
        .unwrap(),
    );

    disk.apply_tag(custom_tag.index(), &comp_nodes[0]).unwrap();
    disk.apply_tag(custom_tag.index(), &comp_nodes[1]).unwrap();
    disk.apply_tag(custom_tag.index(), &comp_nodes[2]).unwrap();

    assert_eq!(disk.list_tag_nodes(custom_tag.index()).unwrap(), comp_nodes);
}

#[test]
fn test_remove_tag() {
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

    let custom_tag = disk
        .create_new_tag("file_1", TagFlags::new(true, true))
        .unwrap();

    assert_eq!(custom_tag.index(), 1);

    let file_contents = "The file contents are testing, 1234, ok so this should be one block!"
        .as_bytes()
        .to_vec();

    let mut comp_nodes = Vec::new();

    comp_nodes.push(
        disk.create_new_file_first_free(
            "test_file",
            INodeFlags::new(true, true, true, false),
            file_contents.clone(),
        )
        .unwrap(),
    );

    let mut ultra_large_file = vec![0u8; 4096 * 6]; // We want to take up more than 5 blocks so an indirect inode is required.
    ultra_large_file[0] = 0xff;

    comp_nodes.push(
        disk.create_new_file_first_free(
            "test_file_2",
            INodeFlags::new(true, true, true, false),
            ultra_large_file,
        )
        .unwrap(),
    );

    let file_contents_2 = "Different file contents for this file!".as_bytes().to_vec();

    comp_nodes.push(
        disk.create_new_file_first_free(
            "test_file",
            INodeFlags::new(true, true, true, false),
            file_contents_2.clone(),
        )
        .unwrap(),
    );

    disk.apply_tag(custom_tag.index(), &comp_nodes[0]).unwrap();
    disk.apply_tag(custom_tag.index(), &comp_nodes[1]).unwrap();
    disk.apply_tag(custom_tag.index(), &comp_nodes[2]).unwrap();

    disk.remove_tag_from_inode(1, &comp_nodes[0]).unwrap();

    assert_eq!(disk.list_tags().len(), 2);
    let inodes = disk.list_tag_nodes(1).unwrap();
    assert_eq!(inodes[0], comp_nodes[1]);
    assert_eq!(inodes[1], comp_nodes[2]);
}

#[test]
fn test_remove_from_indirect_tag() {
    let mut handler = Handler::new(4096 * 60); // Disk size of 120 KiB
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
            disk.create_new_file_first_free(
                "test_file",
                INodeFlags::new(true, true, true, false),
                file_contents.clone(),
            )
            .unwrap(),
        );

        disk.apply_tag(custom_tag.index(), &comp_nodes[i]).unwrap();
    }

    disk.remove_tag_from_inode(custom_tag.index(), &comp_nodes.remove(0)).unwrap();
    disk.remove_tag_from_inode(custom_tag.index(), &comp_nodes.pop().unwrap())
        .unwrap();

    assert_eq!(comp_nodes, disk.list_tag_nodes(custom_tag.index()).unwrap());
}

#[test]
fn test_remove_from_large_tag() {
    let mut handler = Handler::new(4096 * 1000); // Disk size of 120 KiB
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
            disk.create_new_file_first_free(
                "test_file",
                INodeFlags::new(true, true, true, false),
                file_contents.clone(),
            )
            .unwrap(),
        );

        disk.apply_tag(custom_tag.index(), &comp_nodes[i]).unwrap();
    }

    assert_eq!(disk.available_data_blocks(), 349);

    // We will remove all the middle block.
    for _ in 0..509 {
        disk.remove_tag_from_inode(custom_tag.index(), &comp_nodes.remove(12));
    }

    assert_eq!(disk.available_data_blocks(), 350);
    assert_eq!(comp_nodes, disk.list_tag_nodes(custom_tag.index()).unwrap());
}

#[test]
fn test_delete_tag() {
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

    let custom_tag = disk
        .create_new_tag("file_1", TagFlags::new(true, true))
        .unwrap();

    assert_eq!(custom_tag.index(), 1);

    let file_contents = "The file contents are testing, 1234, ok so this should be one block!"
        .as_bytes()
        .to_vec();

    let mut comp_nodes = Vec::new();

    comp_nodes.push(
        disk.create_new_file_first_free(
            "test_file",
            INodeFlags::new(true, true, true, false),
            file_contents.clone(),
        )
        .unwrap(),
    );

    let mut ultra_large_file = vec![0u8; 4096 * 6]; // We want to take up more than 5 blocks so an indirect inode is required.
    ultra_large_file[0] = 0xff;

    comp_nodes.push(
        disk.create_new_file_first_free(
            "test_file_2",
            INodeFlags::new(true, true, true, false),
            ultra_large_file,
        )
        .unwrap(),
    );

    let file_contents_2 = "Different file contents for this file!".as_bytes().to_vec();

    comp_nodes.push(
        disk.create_new_file_first_free(
            "test_file",
            INodeFlags::new(true, true, true, false),
            file_contents_2.clone(),
        )
        .unwrap(),
    );

    disk.apply_tag(custom_tag.index(), &comp_nodes[0]).unwrap();
    disk.apply_tag(custom_tag.index(), &comp_nodes[1]).unwrap();
    disk.apply_tag(custom_tag.index(), &comp_nodes[2]).unwrap();

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
            disk.create_new_file_first_free(
                "test_file",
                INodeFlags::new(true, true, true, false),
                file_contents.clone(),
            )
            .unwrap(),
        );

        disk.apply_tag(custom_tag.index(), &comp_nodes[i]).unwrap();
    }

    disk.delete_tag(custom_tag.index()).unwrap();

    assert_eq!(disk.list_tags().len(), 1);

    let custom_tag_2 = disk
        .create_new_tag("file_2", TagFlags::new(true, true))
        .unwrap(); // Create a new tag

    assert_eq!(custom_tag_2.index(), 1);
}
