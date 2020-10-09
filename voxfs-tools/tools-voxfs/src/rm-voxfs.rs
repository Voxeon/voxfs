use clap::{App, Arg};
use std::io::Write;
use std::process::exit;
use voxfs::Disk;
use voxfs_tool_lib::{Handler, Manager};

fn main() {
    let arguments = App::new("rm-voxfs")
        .version("0.1.0")
        .about("This program removes files from a voxfs image.")
        .arg(
            Arg::with_name("image")
                .required(true)
                .takes_value(true)
                .help("The path of the image"),
        )
        .arg(
            Arg::with_name("file")
                .required(true)
                .takes_value(true)
                .help("The path of the file to remove"),
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

    let mut disk = match Disk::open_disk(&mut handler, &mut manager) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Disk opening error: {:?}", e);
            exit(1);
        }
    };

    let file_name = match arguments.value_of("file") {
        Some(f) => f,
        None => {
            eprintln!("A file to remove is required.");
            exit(1);
        }
    };

    print!(
        "Are you sure you wish to remove \"{}\" from the image: (y/n) ",
        file_name
    );

    match std::io::stdout().flush() {
        Ok(_) => (),
        Err(_) => (),
    }

    let mut input = String::new();
    match std::io::stdin().read_line(&mut input) {
        Ok(_) => (),
        Err(_) => {
            eprintln!("Failed to read response.");
            exit(1);
        }
    }

    input = input.strip_suffix("\n").unwrap_or("").to_string();
    if input.ends_with('\r') {
        input = input.strip_suffix("\r").unwrap_or("").to_string();
    }

    if input != "y" && input != "Y" {
        println!("Will not remove file.");
        exit(0);
    }

    let index = match disk.inode_with_name(file_name) {
        Some(i) => i,
        None => {
            eprintln!("Could not find file with name {}", file_name);
            exit(1);
        }
    };

    match disk.delete_file(index) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Could not remove file due to error: {}", e);
            exit(1);
        }
    }

    println!("Successfully removed file!");
}
