use crate::backend::TerminalBackend;
use crate::colors::TerminalPalette;
use crate::pty::PtyHandle;

use alacritty_terminal::term::cell::Flags as CellFlags;
use alacritty_terminal::vte::ansi::CursorShape;

use iced::alignment;
use iced::widget::canvas::{self, Frame, Geometry, Text as CanvasText};
use iced::{mouse, Color, Font, Pixels, Point, Rectangle, Renderer, Size, Theme};

use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Shared terminal state between the widget and the async reader.
pub struct TerminalState {
    pub backend: TerminalBackend,
    pub pty: PtyHandle,
    pub palette: TerminalPalette,
}

impl TerminalState {
    pub fn new(
        cols: u16,
        rows: u16,
    ) -> Result<(Self, mpsc::UnboundedReceiver<Vec<u8>>), Box<dyn std::error::Error + Send + Sync>>
    {
        let backend = TerminalBackend::new(cols, rows);
        let (pty, rx) = PtyHandle::spawn(cols, rows)?;
        let palette = TerminalPalette::default();

        Ok((Self { backend, pty, palette }, rx))
    }

    /// Feed PTY output bytes into the terminal.
    pub fn process(&mut self, bytes: &[u8]) {
        self.backend.process(bytes);
    }

    /// Write keyboard input to PTY.
    pub fn write(&mut self, data: &[u8]) {
        if let Err(e) = self.pty.write(data) {
            tracing::error!("PTY write error: {}", e);
        }
    }
}

/// The Iced Canvas program that renders the terminal grid.
pub struct TerminalView {
    state: Arc<Mutex<TerminalState>>,
    cell_width: f32,
    cell_height: f32,
}

impl TerminalView {
    pub fn new(state: Arc<Mutex<TerminalState>>) -> Self {
        Self {
            state,
            cell_width: 8.4,
            cell_height: 18.0,
        }
    }

    /// Calculate grid dimensions from pixel size.
    pub fn grid_size_for(width: f32, height: f32) -> (u16, u16) {
        let cell_width = 8.4_f32;
        let cell_height = 18.0_f32;
        let cols = (width / cell_width).floor().max(1.0) as u16;
        let rows = (height / cell_height).floor().max(1.0) as u16;
        (cols, rows)
    }
}

impl<Message> canvas::Program<Message, Theme> for TerminalView
where
    Message: Clone,
{
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let state = self.state.lock().unwrap();
        let term = &state.backend.term;
        let palette = &state.palette;
        let colors = term.colors();

        let mut frame = Frame::new(renderer, bounds.size());

        // Background
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), palette.background);

        let cell_w = self.cell_width;
        let cell_h = self.cell_height;

        let content = term.renderable_content();

        for item in content.display_iter {
            let cell = item.cell;
            let point = item.point;

            // Skip spacer cells for wide characters
            if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
                continue;
            }

            let x = point.column.0 as f32 * cell_w;
            let y = point.line.0 as f32 * cell_h;

            // Resolve colors
            let mut fg = palette.resolve(&cell.fg, colors);
            let mut bg = palette.resolve(&cell.bg, colors);

            // Handle inverse
            if cell.flags.contains(CellFlags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }

            // Handle dim
            if cell.flags.contains(CellFlags::DIM) {
                fg = Color::from_rgba(fg.r * 0.66, fg.g * 0.66, fg.b * 0.66, fg.a);
            }

            // Draw cell background (only if not default bg)
            let is_default_bg = (bg.r - palette.background.r).abs() < 0.01
                && (bg.g - palette.background.g).abs() < 0.01
                && (bg.b - palette.background.b).abs() < 0.01;

            if !is_default_bg {
                let width = if cell.flags.contains(CellFlags::WIDE_CHAR) {
                    cell_w * 2.0
                } else {
                    cell_w
                };
                frame.fill_rectangle(Point::new(x, y), Size::new(width, cell_h), bg);
            }

            // Draw character
            let c = cell.c;
            if c != ' ' && c != '\0' {
                frame.fill_text(CanvasText {
                    content: c.to_string(),
                    position: Point::new(x, y),
                    color: fg,
                    size: Pixels(14.0),
                    font: Font::MONOSPACE,
                    align_x: alignment::Horizontal::Left.into(),
                    align_y: alignment::Vertical::Top,
                    ..Default::default()
                });
            }

            // Draw underline
            if cell.flags.intersects(CellFlags::ALL_UNDERLINES) {
                let underline_y = y + cell_h - 2.0;
                let width = if cell.flags.contains(CellFlags::WIDE_CHAR) {
                    cell_w * 2.0
                } else {
                    cell_w
                };
                frame.fill_rectangle(Point::new(x, underline_y), Size::new(width, 1.0), fg);
            }

            // Draw strikethrough
            if cell.flags.contains(CellFlags::STRIKEOUT) {
                let strike_y = y + cell_h / 2.0;
                let width = if cell.flags.contains(CellFlags::WIDE_CHAR) {
                    cell_w * 2.0
                } else {
                    cell_w
                };
                frame.fill_rectangle(Point::new(x, strike_y), Size::new(width, 1.0), fg);
            }
        }

        // Draw cursor
        let cursor = content.cursor;
        let cursor_x = cursor.point.column.0 as f32 * cell_w;
        let cursor_y = cursor.point.line.0 as f32 * cell_h;

        match cursor.shape {
            CursorShape::Block => {
                frame.fill_rectangle(
                    Point::new(cursor_x, cursor_y),
                    Size::new(cell_w, cell_h),
                    Color { a: 0.7, ..palette.cursor },
                );
            }
            CursorShape::Beam => {
                frame.fill_rectangle(
                    Point::new(cursor_x, cursor_y),
                    Size::new(2.0, cell_h),
                    palette.cursor,
                );
            }
            CursorShape::Underline => {
                frame.fill_rectangle(
                    Point::new(cursor_x, cursor_y + cell_h - 2.0),
                    Size::new(cell_w, 2.0),
                    palette.cursor,
                );
            }
            _ => {
                frame.fill_rectangle(
                    Point::new(cursor_x, cursor_y),
                    Size::new(cell_w, cell_h),
                    Color { a: 0.5, ..palette.cursor },
                );
            }
        }

        vec![frame.into_geometry()]
    }
}
