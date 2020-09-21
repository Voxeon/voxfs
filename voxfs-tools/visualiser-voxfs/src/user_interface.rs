use crate::error::VisualiserError;
use std::io;
use std::io::Stdout;
use tui::backend::CrosstermBackend;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Row, Table, TableState};
use tui::Terminal;

type TerminalBackend = CrosstermBackend<Stdout>;

pub struct UI {
    terminal: Terminal<TerminalBackend>,
    highlight_style: Style,
    default_style: Style,
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
        });
    }

    pub fn render_main_menu(&mut self, selected_index: usize) -> Result<(), VisualiserError> {
        let default_style = self.default_style;
        let highlight_style = self.highlight_style;

        ignore_result!(self.force_redraw_next_frame());

        match self.terminal.draw(|f| {
            let items = [
                ListItem::new("Navigate Disk"),
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
        selected_address: u64,
    ) -> Result<(), VisualiserError> {
        let default_style = self.default_style;
        let highlight_style = self.highlight_style;

        ignore_result!(self.force_redraw_next_frame());

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
            //state.select(Some((cell_x * cell_y) as usize));

            // let block = List::new(items)
            //     .block(Block::default().title("Main Menu").borders(Borders::ALL))
            //     .style(Style::default().fg(Color::White))
            //     .highlight_style(
            //         Style::default()
            //             .add_modifier(Modifier::ITALIC)
            //             .add_modifier(Modifier::REVERSED),
            //     )
            //     .highlight_symbol(">> ");

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
            let mut current_row = Vec::new();

            for byte in bytes {
                Row::Data(["Row41", "ff", "ff", "ef"].into_iter());
                if current_row.len() >= 16 {}

            }

            let mut widths = [Constraint::Length(2); 17];
            widths[0] = Constraint::Length(10);

            let block = Table::new(
                header.iter(),
                rows.into_iter(),
            )
            .column_spacing(2)
            .block(Block::default().title("Disk Contents").borders(Borders::ALL))
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
            f.render_widget(block, rects[0])
            //f.render_stateful_widget(t, f.size(), &mut state);
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
}
