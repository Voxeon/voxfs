use crate::{VisualiserError, UI};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::path::Path;
use voxfs::{Disk, DiskHandler};
use voxfs_tool_lib::{Handler, MKImageError, Manager};
use std::process::exit;

enum CurrentMenu {
    Main,
    RawDiskRoot,
}

pub struct Application {
    path: String,
    disk_size: Option<u64>,
    current_menu: CurrentMenu,
    quit: bool,
    ui: UI,
}

impl Application {
    pub fn new(image_path: String) -> Result<Self, VisualiserError> {
        if !Path::new(&image_path).exists() {
            return Err(VisualiserError::new(&format!(
                "No image file found with path: {}",
                image_path
            )));
        }

        return Ok(Self {
            path: image_path,
            disk_size: None,
            current_menu: CurrentMenu::Main,
            quit: false,
            ui: UI::new()?,
        });
    }

    pub fn run(&mut self) -> Result<(), VisualiserError> {
        let mut handler = match Handler::new(self.path.clone()) {
            Ok(h) => h,
            Err(e) => {
                return Err(VisualiserError::new(&e.get_message()));
            }
        };

        match handler.disk_size() {
            Ok(sz) => {
                self.disk_size = Some(sz);
            }
            Err(_) => return Err(VisualiserError::new("Failed to retrieve disk size.")),
        }

        let mut manager = Manager::new();

        let disk = match Disk::open_disk(&mut handler, &mut manager) {
            Ok(d) => d,
            Err(_) => return Err(VisualiserError::new_internal("Failed to open disk.")),
        };

        match enable_raw_mode() {
            Ok(_) => (),
            Err(_) => {
                return Err(VisualiserError::new(
                    "Couldn't enable raw mode for the terminal.",
                ))
            }
        }

        let res = self.main_loop(disk);
        self.ui.try_clear();
        ignore_result!(disable_raw_mode());
        self.ui.show_cursor();

        return res;
    }

    fn main_loop(&mut self, mut disk: Disk<MKImageError>) -> Result<(), VisualiserError> {
        while !self.quit {
            match self.current_menu {
                CurrentMenu::Main => self.main_menu()?,
                CurrentMenu::RawDiskRoot => self.raw_disk_root(&mut disk)?,
            }
        }

        return Ok(());
    }

    fn raw_disk_root(&mut self, disk: &mut Disk<MKImageError>) -> Result<(), VisualiserError> {
        // NOTE: We can assume disk_size is not None because we must have a disk size to call this method before hand
        let mut cont = true;
        let mut starting_address = 0;

        let (max_cols, max_rows) = match UI::get_size() {
            Some(p) => p,
            None => {
                return Err(VisualiserError::new_internal(
                    "Couldn't determine the terminal's size",
                ))
            }
        };

        let bytes_per_render = (max_cols as usize) * (max_rows as usize);

        let mut current_address = 0;

        while cont {
            let start = starting_address;
            let mut end = start + bytes_per_render;

            if end > self.disk_size.unwrap() as usize {
                end = self.disk_size.unwrap() as usize;
            }

            let bytes = match disk
                .handler()
                .read_bytes(start as u64, (end - start) as u64)
            {
                Ok(b) => b,
                Err(_) => return Err(VisualiserError::new("Could not read the file.")),
            };

            self.ui
                .render_raw_disk_ui(&bytes, 0, current_address)?;

            let key = self.blocking_read_key()?;
            match key {
                Some(k) => {
                    if k.code == KeyCode::Char('q') {
                        return Ok(());
                    }
                }
                None => (),
            }
        }

        return Ok(());
    }

    fn main_menu(&mut self) -> Result<(), VisualiserError> {
        let mut cont = true;
        let mut selected_index = 0; // Quit is the last index
        let number_of_options = 3;

        while cont {
            self.ui.render_main_menu(selected_index)?;
            let event = self.blocking_read_key()?;

            match event {
                Some(k) => {
                    if k.code == KeyCode::Enter {
                        if selected_index == number_of_options - 1 {
                            self.quit = true;
                            cont = false; // Time to quit
                        } else if selected_index == 1 {
                            self.current_menu = CurrentMenu::RawDiskRoot;
                            cont = false;
                        }
                    } else if k.code == KeyCode::Down {
                        selected_index += 1;
                        selected_index %= number_of_options;
                    } else if k.code == KeyCode::Up {
                        if selected_index == 0 {
                            selected_index = number_of_options - 1;
                        } else {
                            selected_index -= 1;
                        }
                    }
                }
                None => (),
            }
        }

        return Ok(());
    }

    fn blocking_read_key(&mut self) -> Result<Option<KeyEvent>, VisualiserError> {
        let event = match crossterm::event::read() {
            Ok(e) => e,
            Err(e) => return Err(VisualiserError::new(&format!("{}", e))),
        };

        let key = match event {
            Event::Key(kv) => kv,
            _ => return Ok(None),
        };

        if key == KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL) {
            return Err(VisualiserError::new("SIGINT was found"));
        } else {
            return Ok(Some(key));
        }
    }
}
