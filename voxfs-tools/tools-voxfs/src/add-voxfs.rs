use clap::{App, Arg};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::process::exit;
use voxfs::{Disk, INodeFlags};
use voxfs_tool_lib::{Handler, Manager};

const BUFFER_SIZE: usize = 4000;

fn main() {
    let arguments = App::new("add-voxfs")
        .version("0.1.0")
        .about("This program adds files to a voxfs image.")
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
                .help("The path of the file to add"),
        )
        .arg(
            Arg::with_name("name")
                .short("n")
                .long("name")
                .takes_value(true)
                .help("The name of the file as it should be stored in the voxfs image."),
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

    let file_path = match arguments.value_of("file") {
        Some(f) => f.to_string(),
        None => {
            eprintln!("A file to add is required.");
            exit(1);
        }
    };

    let name = match arguments.value_of("name") {
        Some(n) => n.to_string(),
        None => match Path::new(&file_path).file_name() {
            Some(n) => match n.to_str() {
                Some(n) => n.to_string(),
                None => {
                    eprintln!("Could not determine a file name to use for the image.");
                    exit(1);
                }
            },
            None => {
                eprintln!("Could not determine a file name to use for the image.");
                exit(1);
            }
        },
    };

    print!(
        "Are you sure you wish to copy \"{}\" into the image as \"{}\": (y/n) ",
        file_path, name
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
        println!("Will not add file.");
        exit(0);
    }

    let mut file = match File::open(file_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Could not open file due to error: {}", e);
            exit(1);
        }
    };

    let mut buffer = vec![0u8; BUFFER_SIZE];

    let mut amount_read = match file.read(&mut buffer) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error while reading: {}", e);
            exit(1);
        }
    };

    let file_index =
        match disk.create_new_file(&name, INodeFlags::default(), buffer[..amount_read].to_vec()) {
            Ok(i) => i.index(),
            Err(e) => {
                eprintln!("Error: {}", e);
                exit(1);
            }
        };

    amount_read = match file.read(&mut buffer) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error while reading: {}", e);
            exit(1);
        }
    };

    while amount_read > 0 {
        match disk.append_file_bytes(file_index, &buffer[..amount_read].to_vec()) {
            Ok(_) => (),
            Err(e) => {
                eprintln!("Error: {}", e);
                exit(1);
            }
        }

        amount_read = match file.read(&mut buffer) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error while reading: {}", e);
                exit(1);
            }
        };
    }

    println!("Successfully added file!");
}
