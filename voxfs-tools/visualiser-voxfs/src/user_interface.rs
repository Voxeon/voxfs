use crate::error::VisualiserError;
use std::io;
use std::io::Stdout;
use tui::backend::CrosstermBackend;
use tui::layout::{Constraint, Direction, Layout, Alignment};
use tui::style::{Color, Modifier, Style};
use tui::text::{Span, Spans, Text};
use tui::widgets::{
    Block, Borders, List, ListItem, ListState, Paragraph, Row, Table, TableState,
};
use tui::Terminal;
use voxfs::DiskInfo;
use voxfs_tool_lib::u64_to_sized_string;

type TerminalBackend = CrosstermBackend<Stdout>;

pub struct UI {
    terminal: Terminal<TerminalBackend>,
    highlight_style: Style,
    default_style: Style,
    error_style: Style,
}

impl UI {
    pub fn new() -> Result<Self, VisualiserError> {
        let stdout = io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let terminal = match Terminal::new(backend) {
            Ok(t) => t,
            Err(e) => {
                return Err(VisualiserError::new_internal(&format!(
                    "Terminal creation error: {}",
                    e
                )))
            }
        };

        return Ok(Self {
            terminal,
            default_style: Style::default().fg(Color::White),
            highlight_style: Style::default()
                .add_modifier(Modifier::ITALIC)
                .add_modifier(Modifier::REVERSED),
            error_style: Style::default()
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::SLOW_BLINK)
                .fg(Color::Red),
        });
    }

    pub fn render_main_menu(
        &mut self,
        selected_index: usize,
        force_redraw: bool,
    ) -> Result<(), VisualiserError> {
        let default_style = self.default_style;
        let highlight_style = self.highlight_style;

        if force_redraw {
            ignore_result!(self.force_redraw_next_frame());
        }

        match self.terminal.draw(|f| {
            let items = [
                ListItem::new("Disk Information"),
                ListItem::new("View Raw Disk"),
                ListItem::new("Quit"),
            ];

            let mut state = ListState::default();
            state.select(Some(selected_index));

            let block = List::new(items)
                .block(Block::default().title("Main Menu").borders(Borders::ALL))
                .style(default_style)
                .highlight_style(highlight_style)
                .highlight_symbol(">> ");

            f.render_stateful_widget(block, f.size(), &mut state);
        }) {
            Ok(_) => (),
            Err(e) => {
                return Err(VisualiserError::new_internal(&format!(
                    "Failed to render menu. Error: {}",
                    e
                )))
            }
        }

        return Ok(());
    }

    pub fn render_raw_disk_ui(
        &mut self,
        bytes: &Vec<u8>,
        current_offset: u64,
        selected_row: usize,
        force_redraw: bool,
    ) -> Result<(), VisualiserError> {
        let default_style = self.default_style;
        let highlight_style = self.highlight_style;

        if force_redraw {
            ignore_result!(self.force_redraw_next_frame());
        }

        match self.terminal.draw(|f| {
            let rects = Layout::default()
                .constraints([Constraint::Min(10), Constraint::Length(3)])
                .direction(Direction::Vertical)
                .split(f.size());

            // We want to render an interface like this (NOTE this won't allow addresses over 32 bits)
            //
            // Offset      00  01  02  03  04  05  06  07  08  09  0a  0b  0c  0d  0e  0f
            // 00000000    xx  xx  xx  xx  xx  xx  xx  xx  xx  xx  xx  xx  xx  xx  xx  xx
            // 00000010    xx  xx  xx  xx  xx  xx  xx  xx  xx  xx  xx  xx  xx  xx  xx  xx

            let mut state = TableState::default();
            state.select(Some(selected_row));

            let header = [
                "Offset    ",
                "00",
                "01",
                "02",
                "03",
                "04",
                "05",
                "06",
                "07",
                "08",
                "09",
                "0a",
                "0b",
                "0c",
                "0d",
                "0e",
                "0f",
            ];

            let mut rows = Vec::new();
            let mut iteration_offset = current_offset;
            let mut current_row = vec![format!("{:08x}", iteration_offset)];

            for byte in bytes {
                //Row::Data(["Row41", "ff", "ff", "ef"].into_iter());
                if current_row.len() >= 17 {
                    rows.push(Row::Data(current_row.into_iter()));
                    iteration_offset += 0x10;
                    current_row = vec![
                        format!("{:08x}", iteration_offset),
                        format!("{:02x}", *byte),
                    ];
                } else {
                    current_row.push(format!("{:02x}", *byte));
                }
            }

            if current_row.len() >= 17 {
                rows.push(Row::Data(current_row.into_iter()));
            } else {
                while current_row.len() < 17 {
                    current_row.push(format!("  "));
                }

                rows.push(Row::Data(current_row.into_iter()));
            }

            let mut widths = [Constraint::Length(2); 17];
            widths[0] = Constraint::Length(10);

            let block = Table::new(header.iter(), rows.into_iter())
                .column_spacing(2)
                .block(
                    Block::default()
                        .title("Disk Contents")
                        .borders(Borders::ALL),
                )
                .style(default_style)
                .highlight_style(highlight_style)
                .widths(&widths);

            let footer_text = vec![Spans::from(vec![
                Span::raw("esc - Back"),
                Span::raw("    "),
                Span::raw("↑,↓,←,→ - Move Cursor"),
            ])];

            let footer_block = Paragraph::new(footer_text)
                .style(default_style)
                .block(Block::default().title("Keys").borders(Borders::ALL));

            f.render_widget(footer_block, rects[1]);
            f.render_stateful_widget(block, rects[0], &mut state);
        }) {
            Ok(_) => (),
            Err(e) => {
                return Err(VisualiserError::new_internal(&format!(
                    "Failed to render menu. Error: {}",
                    e
                )))
            }
        }

        return Ok(());
    }

    pub fn render_disk_info(&mut self, disk_info: &DiskInfo, force_redraw: bool) -> Result<(), VisualiserError> {
        let default_style = self.default_style;

        if force_redraw {
            ignore_result!(self.force_redraw_next_frame());
        }

        match self.terminal.draw(|f| {
            let splits = Layout::default().constraints(vec![Constraint::Min(10), Constraint::Length(3)]).direction(Direction::Vertical).split(f.size());
            let body = Paragraph::new(Text::raw(format!("Tags: {}\nNumber of Free Tags: {}\nFiles: {}\nFree File spaces: {}\nBlock Size: {}\nFree Blocks: {}\n Free space: {}", disk_info.number_of_tags(), disk_info.free_tag_slots(), disk_info.number_of_files(), disk_info.free_file_slots(), disk_info.block_size(), disk_info.free_block_count(), u64_to_sized_string(disk_info.free_block_space())))).block(Block::default().title("Disk Information").borders(Borders::ALL).style(default_style)).alignment(Alignment::Center);
            let command_bar = Paragraph::new(Text::raw("q - return to menu")).block(Block::default().borders(Borders::ALL).style(default_style)).alignment(Alignment::Center);

            f.render_widget(body,splits[0]);
            f.render_widget(command_bar, splits[1]);
        }) {
            Ok(_) => (),
            Err(e) => {
                return Err(VisualiserError::new_internal(&format!(
                    "Failed to render menu. Error: {}",
                    e
                )))
            }
        }

        return Ok(());
    }

    /// This code will render a file name prompt, it's currently not used but was written and kept for potential future use
    pub fn render_file_name_prompt(
        &mut self,
        file_name: &str,
        error_message: &Option<String>,
        force_redraw: bool,
    ) -> Result<(), VisualiserError> {
        let default_style = self.default_style;
        let error_style = self.error_style;

        if force_redraw {
            ignore_result!(self.force_redraw_next_frame());
        }

        match self.terminal.draw(|f| {
            let height;

            if error_message.is_none() {
                height = 3;
            } else {
                height = 4
            }

            let vert_rects = Layout::default()
                .constraints([
                    Constraint::Length((f.size().height - height) / 2),
                    Constraint::Length(height),
                    Constraint::Length((f.size().height - height) / 2),
                ])
                .direction(Direction::Vertical)
                .split(f.size());

            let horiz_rects = Layout::default()
                .constraints([
                    Constraint::Percentage(10),
                    Constraint::Percentage(80),
                    Constraint::Percentage(10),
                ])
                .direction(Direction::Horizontal)
                .split(vert_rects[1]);

            let padding_block_1 = Block::default();
            let padding_block_2 = Block::default();
            let padding_block_3 = Block::default();
            let padding_block_4 = Block::default();

            let spans;

            match error_message {
                Some(e) => {
                    spans = vec![
                        Spans::from(vec![Span::styled(e, error_style)]),
                        Spans::from(Span::styled(file_name, default_style)),
                    ];
                }
                None => {
                    spans = vec![Spans::from(vec![Span::styled(file_name, default_style)])];
                }
            }

            let mut content_block = Paragraph::new(Text::from(spans)).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("File Name")
                    .style(default_style),
            );

            if file_name.len() > (horiz_rects[1].width - 2) as usize {
                content_block =
                    content_block.scroll((0, file_name.len() as u16 - (horiz_rects[1].width - 2)))
            }

            f.render_widget(padding_block_1, vert_rects[0]);
            f.render_widget(padding_block_2, vert_rects[vert_rects.len() - 1]);
            f.render_widget(padding_block_3, horiz_rects[0]);
            f.render_widget(padding_block_4, horiz_rects[2]);
            f.render_widget(content_block, horiz_rects[1]);
            let x = {
                if file_name.len() > (horiz_rects[1].width - 3) as usize {
                    horiz_rects[1].x + 1 + horiz_rects[1].width - 3
                } else {
                    horiz_rects[1].x + 1 + file_name.len() as u16
                }
            };
            f.set_cursor(x, horiz_rects[1].y + horiz_rects[1].height - 2);
        }) {
            Ok(_) => (),
            Err(e) => {
                return Err(VisualiserError::new_internal(&format!(
                    "Failed to render menu. Error: {}",
                    e
                )))
            }
        }

        self.show_cursor();

        return Ok(());
    }

    /// Causes the next frame to be redrawn completely from scratch
    fn force_redraw_next_frame(&mut self) -> std::io::Result<()> {
        return self.terminal.resize(self.terminal.size()?);
    }

    /// Tries to clear the screen. Not guaranteed to succeed and no error will be reported if it fails.
    pub fn try_clear(&mut self) {
        ignore_result!(self.terminal.clear());
        ignore_result!(self.terminal.flush());
    }

    /// Returns the terminal dimensions. Returns (cols, rows).
    pub fn get_size() -> Option<(u16, u16)> {
        match crossterm::terminal::size() {
            Ok(t) => return Some(t),
            Err(_) => return None,
        }
    }

    /// Shows the cursor, ignoring any errors
    pub fn show_cursor(&mut self) {
        ignore_result!(self.terminal.show_cursor());
    }

    /// Hides the cursor, ignoring any errors
    #[allow(dead_code)]
    pub fn hide_cursor(&mut self) {
        ignore_result!(self.terminal.hide_cursor());
    }
}
