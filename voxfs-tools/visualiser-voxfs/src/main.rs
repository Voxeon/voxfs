use clap::{App, Arg};
use std::process::exit;
use visualiser_voxfs::Application;

fn main() {
    let arguments = App::new("visualiser-voxfs")
        .version("0.1.0")
        .about("This program allows for interaction with a voxfs disk image")
        .arg(
            Arg::with_name("path")
                .required(true)
                .takes_value(true)
                .help("The path of the image to open"),
        )
        .get_matches();

    let path = match arguments.value_of("path") {
        Some(p) => p.to_string(),
        None => {
            eprintln!("No path was specified.");
            exit(1);
        }
    };

    let mut application = match Application::new(path) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error: {}", e);
            exit(1);
        }
    };

    match application.run() {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Error: {}", e);
            exit(1);
        }
    }
}
