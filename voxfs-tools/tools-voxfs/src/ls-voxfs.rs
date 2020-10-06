use clap::{App, Arg};
use std::process::exit;
use voxfs::{Disk, VoxFSError};
use voxfs_tool_lib::{u64_to_sized_string, Handler, Manager};

const SPACER: &str = "    ";

fn main() {
    let arguments = App::new("ls-voxfs")
        .version("0.1.0")
        .about("This program lists files in a voxfs image.")
        .arg(
            Arg::with_name("image")
                .required(true)
                .takes_value(true)
                .help("The path of the image"),
        )
        .arg(
            Arg::with_name("filter-tags")
                .short("f")
                .required(false)
                .takes_value(true)
                .multiple(true)
                .help("The tags to apply as a filter when listing."),
        )
        .arg(
            Arg::with_name("list")
                .short("l")
                .required(false)
                .takes_value(false)
                .help("List the files with their metadata."),
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
            eprintln!("An error occurred: {:?}", e);
            exit(1);
        }
    };

    if arguments.is_present("filter-tags") {
        let tags: Vec<String> = match arguments.values_of("filter-tags") {
            Some(t) => t.map(|s| s.to_string()).collect(),
            None => Vec::new(),
        };

        let tags_len = tags.len();

        let indices = match disk.tags_with_names(tags) {
            Ok(i) => i,
            Err(e) => {
                match e {
                    VoxFSError::NoTagsWithNames(_) => {
                        eprintln!("{}", e);
                    }
                    VoxFSError::MoreNamesThanTagsProvided => {
                        eprintln!("There are less than {} tags.", tags_len);
                    }
                    _ => eprintln!("Unexpected error: \"{}\"", e),
                }

                exit(1);
            }
        };

        let inodes = match disk.list_nodes_with_tags(indices) {
            Ok(i) => i,
            Err(e) => {
                match e {
                    _ => eprintln!("An unexpected error occurred: \"{}\"", e),
                }

                exit(1);
            }
        };

        if arguments.is_present("list") {
            if inodes.len() == 0 {
                println!("No files were found that satisfied the criteria.");
            }

            for i in 0..inodes.len() {
                println!(
                    "{:<6}{}{:<12}{}{}",
                    u64_to_sized_string(inodes[i].file_size()),
                    SPACER,
                    inodes[i].access_time().format("%b %H:%M:%S"),
                    SPACER,
                    inodes[i].name()
                );
            }
        } else {
            if inodes.len() == 0 {
                println!("No files were found that satisfied the criteria.");
            }

            for i in 0..inodes.len() {
                if i != 0 && (i + 1) % 3 == 0 {
                    println!("{}", inodes[i].name());
                } else {
                    print!("{}{}", inodes[i].name(), SPACER);
                }
            }
        }
    } else if arguments.is_present("list") {
        let inodes = disk.list_inodes();

        for i in 0..inodes.len() {
            println!(
                "{:<6}{}{:<12}{}{}",
                u64_to_sized_string(inodes[i].file_size()),
                SPACER,
                inodes[i].access_time().format("%b %H:%M:%S"),
                SPACER,
                inodes[i].name()
            );
        }
    } else {
        let inodes = disk.list_inodes();

        for i in 0..inodes.len() {
            if i != 0 && (i + 1) % 3 == 0 {
                println!("{}", inodes[i].name());
            } else {
                print!("{}{}", inodes[i].name(), SPACER);
            }
        }
    }
}
