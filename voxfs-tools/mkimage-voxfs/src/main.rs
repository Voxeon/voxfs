use clap::{App, Arg};
use std::io::Write;
use std::path::Path;
use std::process::exit;
use voxfs::Disk;
use voxfs_tool_lib::{sized_string_to_u64, Handler, Manager};

fn main() {
    let arguments = App::new("mkimage-voxfs")
        .version("0.1.0")
        .about("This program creates an image with a voxfs filesystem")
        .arg(
            Arg::with_name("path")
                .required(true)
                .takes_value(true)
                .help("The path of the image"),
        )
        .arg(
            Arg::with_name("size")
                .required(true)
                .takes_value(true)
                .help("The size of the image with optional (KB, MB, GB)."),
        )
        .get_matches();

    let path = match arguments.value_of("path") {
        Some(p) => p,
        None => {
            eprintln!("A path is required.");
            exit(1);
        }
    };

    let size_str = match arguments.value_of("size") {
        Some(p) => p,
        None => {
            eprintln!("A size is required.");
            exit(1);
        }
    };

    let size = match sized_string_to_u64(size_str) {
        Some(s) => s,
        None => {
            eprintln!("A valid integer size is required.");
            exit(1);
        }
    };

    if size < 40_960 {
        eprintln!("Image size must be atleast 40KB.");
        exit(1);
    }

    println!("Create image of size {} bytes at {}", size, path);
    print!("Confirm (y/N) ");

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

    // strip the new line character
    input = input.strip_suffix("\n").unwrap_or("").to_string();

    if input != "y" && input != "Y" {
        println!("Did not create image.");
        exit(0);
    }

    // Check if file already exists.
    let path_struct = Path::new(path);

    if path_struct.exists() {
        println!("A file already exists at {}", &path);
        print!("Delete file (y/N) ");

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

        // strip the new line character
        input = input.strip_suffix("\n").unwrap_or("").to_string();

        if input != "y" && input != "Y" {
            println!("Did not create image.");
            exit(0);
        }

        // Delete the file
        match std::fs::remove_file(path) {
            Ok(_) => (),
            Err(_) => {
                eprintln!("Could not delete the old file.");
                exit(1);
            }
        }
    }

    let mut handler = match Handler::new_create(path.to_string(), size as usize) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("{}", e);
            exit(1);
        }
    };

    let mut manager = Manager::new();

    match Disk::make_new_filesystem(&mut handler, &mut manager) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("{:?}", e);
            exit(1);
        }
    };

    println!("Successfully created image at {}", path);
}
