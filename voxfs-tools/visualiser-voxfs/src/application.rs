use crate::{VisualiserError, UI};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::path::Path;
use voxfs::{Disk, DiskHandler, FORBIDDEN_CHARACTERS};
use voxfs_tool_lib::{Handler, MKImageError, Manager};

enum CurrentMenu {
    Main,
    RawDiskRoot,
    DiskInfo,
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
                CurrentMenu::DiskInfo => self.disk_info(&mut disk)?,
            }
        }

        return Ok(());
    }

    fn raw_disk_root(&mut self, disk: &mut Disk<MKImageError>) -> Result<(), VisualiserError> {
        // NOTE: We can assume disk_size is not None because we must have a disk size to call this method before hand
        let mut cont = true;
        let mut starting_address = 0;
        let mut selected_row = 0;
        let mut force_redraw = true;

        while cont {
            let (_, max_rows) = match UI::get_size() {
                Some(p) => p,
                None => {
                    return Err(VisualiserError::new_internal(
                        "Couldn't determine the terminal's size",
                    ))
                }
            };

            let table_rows = max_rows - 7;

            if selected_row >= table_rows {
                selected_row = table_rows - 1;
            }

            let bytes_per_render = (table_rows as usize) * 16;

            let mut start = starting_address;
            let mut end = starting_address + bytes_per_render;

            if end > self.disk_size.unwrap() as usize {
                end = self.disk_size.unwrap() as usize;
                start = end - bytes_per_render;
            }

            let bytes = match disk
                .handler()
                .read_bytes(start as u64, (end - start) as u64)
            {
                Ok(b) => b,
                Err(_) => return Err(VisualiserError::new("Could not read the file.")),
            };

            self.ui.render_raw_disk_ui(
                &bytes,
                start as u64,
                selected_row as usize,
                force_redraw,
            )?;
            force_redraw = false;

            let key = self.blocking_read_key()?;

            match key {
                Some(k) => {
                    if k.code == KeyCode::Esc {
                        cont = false;
                    } else if k.code == KeyCode::Down {
                        if selected_row < table_rows - 1 {
                            selected_row += 1;
                        } else {
                            starting_address += 0x10;
                            if starting_address + bytes_per_render
                                > self.disk_size.unwrap() as usize
                            {
                                starting_address = ((self.disk_size.unwrap() as usize
                                    - bytes_per_render)
                                    & (usize::MAX - 0xf))
                                    as usize;
                            }
                        }
                    }
                }
                None => (),
            }
        }

        self.current_menu = CurrentMenu::Main;

        return Ok(());
    }

    fn main_menu(&mut self) -> Result<(), VisualiserError> {
        let mut cont = true;
        let mut selected_index = 0; // Quit is the last index
        let number_of_options = 3;
        let mut force_redraw = true;

        while cont {
            self.ui.render_main_menu(selected_index, force_redraw)?;
            force_redraw = false;
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
                        } else if selected_index == 0 {
                            self.current_menu = CurrentMenu::DiskInfo;
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

    fn disk_info(&mut self, disk: &mut Disk<MKImageError>) -> Result<(), VisualiserError> {
        let disk_info = disk.disk_info();
        let mut force_redraw = true;
        let mut cont = true;

        while cont {
            self.ui.render_disk_info(&disk_info, force_redraw)?;
            force_redraw = false;

            let input = self.blocking_read_key()?;
            match input {
                Some(k) => {
                    if k.code == KeyCode::Char('q') {
                        cont = false;
                    }
                }
                None => (),
            }
        }

        self.current_menu = CurrentMenu::Main;

        return Ok(());
    }

    /// This runs a prompt for a file name and returns a suitable file name. It's currently unused but could be in future developments.
    #[allow(dead_code)]
    fn prompt_file_name(&mut self) -> Result<Option<String>, VisualiserError> {
        let mut file_name = String::new();
        let mut error_message = None;
        let mut force_redraw = true;
        let mut cancel = false;
        let mut save = false;

        while !cancel && !save {
            self.ui
                .render_file_name_prompt(&file_name, &error_message, force_redraw)?;
            force_redraw = false;

            let input = self.blocking_read_key()?;
            match input {
                Some(k) => {
                    if k.code == KeyCode::Esc {
                        cancel = true;
                    } else if let KeyCode::Char(ch) = k.code {
                        if file_name.len() < 100 {
                            file_name.push(ch);
                        }
                    } else if k.code == KeyCode::Backspace {
                        file_name.pop();
                    } else if k.code == KeyCode::Enter {
                        if file_name.len() > 100 {
                            error_message = Some("File name is too long".to_string());
                        } else {
                            let mut ok = true;

                            for ref ch in file_name.chars() {
                                if FORBIDDEN_CHARACTERS.contains(ch) {
                                    error_message = Some(format!("Forbidden character '{}'", ch));
                                    ok = false;
                                }
                            }

                            save = ok;
                        }
                    }
                }
                None => (),
            }
        }

        if cancel {
            return Ok(None);
        } else if save {
            return Ok(Some(file_name));
        } else {
            return Err(VisualiserError::new_internal(
                "Unexpected invalid option. Couldn't cancel or save.",
            ));
        }
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
