use clap::{App, Arg};
use std::process::exit;
use voxfs_tool_lib::{Manager, Handler};
use voxfs::Disk;

const SEPARATOR: &str = "  ";

fn main() {
    let arguments = App::new("read-voxfs")
        .version("0.1.0")
        .about("This program reads files from a voxfs image.")
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
                .help("The name of the file to read"),
        )
        .arg(
            Arg::with_name("raw")
                .short("r")
                .long("raw")
                .takes_value(false)
                .help("Print raw hex contents.")
        )
        .arg(
            Arg::with_name("hide_header")
                .long("hide-header")
                .takes_value(false)
                .requires("raw")
                .help("Hide the header when printing raw contents.")
        )
        .arg(
            Arg::with_name("no_formatting")
                .long("no-format")
                .takes_value(false)
                .requires("raw")
                .requires("hide_header")
                .help("Disable any formatting of raw output.")
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

    let file_name = match arguments.value_of("file") {
        Some(f) => f,
        None => {
            eprintln!("A file name is required to read.");
            exit(1);
        }
    };

    let index = match disk.inode_with_name(file_name) {
        Some(i) => i,
        None => {
            eprintln!("No file exists with name \"{}\"", file_name);
            exit(1);
        }
    };

    let contents = match disk.read_file(index) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("An error occurred while reading file contents: {}", e);
            exit(1);
        }
    };

    if arguments.is_present("raw") {
        if contents.len() != 0 && !arguments.is_present("hide_header") {
            for i in 0..0xf {
                print!("{:02x}{}", i, SEPARATOR);
            }

            println!("\n");
        }

        if !arguments.is_present("no_formatting") {
            for i in 0..contents.len() {
                if (i+1) % 0xf == 0 {
                    println!("{:x}", contents[i]);
                } else {
                    print!("{:02x}{}", contents[i], SEPARATOR);
                }
            }
        } else {
            for i in 0..contents.len() {
                print!("{:02x}", contents[i]);
            }
        }
    } else {
        match String::from_utf8(contents) {
            Ok(s) => {
                print!("{}", s);
            }
            Err(_) => {
                eprintln!("Could not create UTF-8 text from file contents.");
                exit(1);
            }
        };
    }
}
