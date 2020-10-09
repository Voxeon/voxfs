use clap::{App, Arg};
use std::process::exit;
use voxfs_tool_lib::{Manager, Handler, MKImageError};
use voxfs::{Disk, TagFlags};
// Apply, remove, create, delete, tags

const SEPARATOR: &str = "    ";

fn main() {
    let arguments = App::new("tag-voxfs")
        .version("0.1.0")
        .about("This program manages tags in a voxfs image.")
        .arg(
            Arg::with_name("image")
                .required(true)
                .takes_value(true)
                .help("The path of the image"),
        )
        .arg(
            Arg::with_name("create")
                .short("c")
                .long("create")
                .takes_value(true)
                .max_values(1)
                .value_name("tag_name")
                .conflicts_with_all(&["delete", "list", "apply", "remove"])
                .help("Create a new tag")
        )
        .arg(
            Arg::with_name("delete")
                .short("d")
                .long("delete")
                .takes_value(true)
                .max_values(1)
                .conflicts_with_all(&["create", "list", "apply", "remove"])
                .value_name("tag_name")
                .help("Delete a tag")
        )
        .arg(
            Arg::with_name("list")
                .short("l")
                .long("list")
                .conflicts_with_all(&["create", "delete", "apply", "remove"])
                .help("List all tags")
        )
        .arg(
            Arg::with_name("apply")
                .short("a")
                .long("apply")
                .takes_value(true)
                .value_names(&["tag_name", "file_name"])
                .max_values(2)
                .conflicts_with_all(&["create", "delete", "list", "remove"])
                .help("Apply tag to file")
        )
        .arg(
            Arg::with_name("remove")
                .short("r")
                .long("remove")
                .takes_value(true)
                .value_names(&["tag_name", "file_name"])
                .max_values(2)
                .conflicts_with_all(&["create", "delete", "list", "apply"])
                .help("Remove a tag from a file")
        )
        .get_matches();

    let path = match arguments.value_of("image") {
        Some(p) => p,
        None => {
            eprintln!("An image is required.");
            exit(1);
        }
    };

    let mut manager = Manager::new();
    let mut handler = match Handler::new(path.to_string()) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("{}", e);
            exit(1);
        }
    };

    let disk = match Disk::open_disk(&mut handler, &mut manager) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Disk opening error: {:?}", e);
            exit(1);
        }
    };

    if arguments.is_present("list") {
        list_tags(disk);
        return;
    }else if arguments.is_present("create") {
        let tag_name = match arguments.value_of("create") {
            Some(n) => n,
            None => {
                eprintln!("Error: A name is required for a new tag.");
                exit(1);
            }
        };

        create_new_tag(disk, tag_name);
        return;
    } else if arguments.is_present("delete") {
        let tag_name = match arguments.value_of("delete") {
            Some(n) => n,
            None => {
                eprintln!("Error: A name is required to delete a tag.");
                exit(1);
            }
        };

        delete_tag(disk, tag_name);
        return;
    } else if arguments.is_present("apply") {
        let (tag_name, file_name) = match arguments.values_of("apply") {
            Some(vals) => {
                let vals: Vec<&str> = vals.collect();

                if vals.len() != 2 {
                    eprintln!("Expected only 2 values instead {} were provided", vals.len());
                    exit(1);
                }

                (vals[0], vals[1])
            },
            None => {
                eprintln!("Error: A tag and file name is required to apply a tag.");
                exit(1);
            }
        };

        apply_tag(disk, tag_name, file_name);
        return;
    }
}

fn list_tags(disk: Disk<MKImageError>) {
    let tags = disk.list_tags();

    if tags.len() == 0 {
        println!("No tags on this disk!");
    }

    for i in 0..tags.len() {
        if (i + 1) % 3 != 0 {
            print!("{}{}", tags[i].name_string(), SEPARATOR);
        } else {
            println!("{}", tags[i].name_string());
        }
    }
}

fn create_new_tag(mut disk: Disk<MKImageError>, tag_name: &str) {
    match disk.create_new_tag(tag_name, TagFlags::default()) {
        Ok(t) => {
            println!("Created new tag with name: \"{}\"", t.name_string());
        },
        Err(e) => {
            eprintln!("Error: {}", e);
            exit(1);
        }
    }
}

fn delete_tag(mut disk: Disk<MKImageError>, tag_name: &str) {
    let tag_index = match disk.tag_with_name(tag_name) {
        Some(t) => t,
        None => {
            eprintln!("No tag exists with the name: \"{}\"", tag_name);
            exit(1);
        }
    };

    match disk.delete_tag(tag_index) {
        Ok(_) => {
            println!("Successfully deleted the tag \"{}\"", tag_name);
        },
        Err(e) => {
            eprintln!("Error whilst deleting tag: {}", e);
            exit(1);
        }
    }
}

fn apply_tag(mut disk: Disk<MKImageError>, tag_name: &str, file_name: &str) {
    let tag_index = match disk.tag_with_name(tag_name) {
        Some(t) => t,
        None => {
            eprintln!("No tag with name: \"{}\" found.", tag_name);
            exit(1);
        }
    };

    let file_index = match disk.inode_with_name(file_name) {
        Some(t) => t,
        None => {
            eprintln!("No file with name: \"{}\" found.", file_name);
            exit(1);
        }
    };

    match disk.apply_tag(tag_index, file_index) {
        Ok(_) => {
            println!("Applied tag \"{}\" to \"{}\"", tag_name, file_name);
            return;
        }
        Err(e) => {
            eprintln!("An error occurred while applying a tag: {}", e);
            exit(1);
        }
    }
}