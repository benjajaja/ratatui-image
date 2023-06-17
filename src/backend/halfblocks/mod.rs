use std::{
    cmp::min,
    io::{self, BufWriter},
};

// use crossterm::style::Color as CColor;
use image::{imageops::FilterType, DynamicImage};
use ratatui::{backend::Backend, buffer::Buffer, layout::Rect, style::Color, Terminal};

use super::{round_size_to_cells, StaticBackend};

// Static sixel backend
pub struct StaticHalfBlocks {
    pub data: hanbun::Buffer<Vec<u8>>,
    pub rect: Rect,
}

impl StaticHalfBlocks {
    pub fn from_image<B: Backend>(
        img: DynamicImage,
        terminal: &mut Terminal<B>,
    ) -> Result<Self, io::Error> {
        let font_size = terminal.backend_mut().font_size()?;
        let rect = round_size_to_cells(img.width(), img.height(), font_size);
        // let img = img_resize(&img, font_size, rect);
        let img = img.resize_exact(
            (rect.width as f64 * 1.5) as u32,
            (rect.height as f64 * 1.5) as u32,
            FilterType::Triangle,
        );
        let data = encode(&img, rect);
        Ok(Self { data, rect })
    }
}

pub fn encode(img: &DynamicImage, rect: Rect) -> hanbun::Buffer<Vec<u8>> {
    let writer = BufWriter::with_capacity((rect.width * rect.height) as _, Vec::new());
    let mut buffer = hanbun::Buffer::with_writer(rect.width as _, rect.height as _, ' ', writer);
    // let mut buffer = hanbun::Buffer::new(rect.width as _, rect.height as _, ' ');

    for (y, row) in img.to_rgb8().rows().enumerate() {
        for (x, pixel) in row.enumerate() {
            buffer.color(
                x,
                y,
                hanbun::Color::Rgb {
                    r: pixel[0],
                    g: pixel[1],
                    b: pixel[2],
                },
            );
        }
    }

    buffer
    // let bytes = buffer.writer.into_inner().unwrap();
    // String::from_utf8(bytes).unwrap()
}

impl StaticBackend for StaticHalfBlocks {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let rect = self.rect;
        let render_area = Rect::new(
            area.x,
            area.y,
            min(rect.width, area.width),
            min(rect.height, area.height),
        );

        for (i, cell) in self.data.cells.iter().enumerate() {
            let x = render_area.x + (i as u16 % render_area.width);
            let y = render_area.y + (i as u16 / area.height);
            let c = buf.get_mut(x, y);

            if cell.upper_block.is_some() && cell.lower_block.is_some() {
                if let Some(Some(upper_color)) = cell.upper_block {
                    if let Some(Some(lower_color)) = cell.lower_block {
                        c.set_fg(to_ratatui_color(upper_color))
                            .set_bg(to_ratatui_color(lower_color))
                            .set_char('▀');
                    } else {
                        c.set_bg(to_ratatui_color(upper_color)).set_char('▄');
                    }
                } else if let Some(Some(lower_color)) = cell.lower_block {
                    if let Some(Some(upper_color)) = cell.upper_block {
                        c.set_fg(to_ratatui_color(upper_color))
                            .set_bg(to_ratatui_color(lower_color))
                            .set_char('▀');
                    } else {
                        c.set_bg(to_ratatui_color(lower_color)).set_char('▀');
                    }
                } else {
                    c.set_char('█');
                }
            } else if let Some(upper_block) = cell.upper_block {
                if let Some(upper_color) = upper_block {
                    c.set_fg(to_ratatui_color(upper_color));
                }
                c.set_char('▀');
                if upper_block.is_some() {
                    // queue!(writer, ResetColor).unwrap();
                }
            } else if let Some(lower_block) = cell.lower_block {
                if let Some(lower_color) = lower_block {
                    c.set_fg(to_ratatui_color(lower_color));
                }
                c.set_char('▄');
                if lower_block.is_some() {
                    // queue!(writer, ResetColor).unwrap();
                }
            } else if let Some(char) = &cell.char {
                if let Some(color) = cell.char_color {
                    c.set_fg(to_ratatui_color(color));
                }

                c.set_char(*char);
                if cell.char_color.is_some() {
                    // queue!(writer, ResetColor).unwrap();
                }
            } else {
                unreachable!();
            }
        }
    }
}

fn to_ratatui_color(color: hanbun::Color) -> Color {
    match color {
        hanbun::Color::Reset => Color::Reset,
        hanbun::Color::Black => Color::Black,
        hanbun::Color::DarkRed => Color::Red,
        hanbun::Color::DarkGreen => Color::Green,
        hanbun::Color::DarkYellow => Color::Yellow,
        hanbun::Color::DarkBlue => Color::Blue,
        hanbun::Color::DarkMagenta => Color::Magenta,
        hanbun::Color::DarkCyan => Color::Cyan,
        hanbun::Color::Grey => Color::Gray,
        hanbun::Color::DarkGrey => Color::DarkGray,
        hanbun::Color::Red => Color::LightRed,
        hanbun::Color::Green => Color::LightGreen,
        hanbun::Color::Yellow => Color::LightYellow,
        hanbun::Color::Blue => Color::LightBlue,
        hanbun::Color::Magenta => Color::LightMagenta,
        hanbun::Color::Cyan => Color::LightCyan,
        hanbun::Color::White => Color::White,
        hanbun::Color::Rgb { r, g, b } => Color::Rgb(r, g, b),
        hanbun::Color::AnsiValue(i) => Color::Indexed(i),
    }
}
