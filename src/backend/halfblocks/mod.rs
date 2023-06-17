// use crossterm::style::Color as CColor;
use image::{imageops::FilterType, DynamicImage};
use ratatui::{buffer::Buffer, layout::Rect, style::Color};

use super::FixedBackend;
use crate::{ImageSource, Resize, Result};

pub mod resizeable;

// Fixed Halfblocks backend
#[derive(Clone, Default)]
pub struct FixedHalfblocks {
    data: Vec<HalfBlock>,
    rect: Rect,
}

#[derive(Clone)]
struct HalfBlock {
    upper: Color,
    lower: Color,
}

impl FixedHalfblocks {
    /// Create a FixedHalfblocks from an image.
    ///
    /// The "resolution" is determined by the font size of the terminal. Smaller fonts will result
    /// in more half-blocks for the same image size. To get a size independent of the font size,
    /// the image could be resized in relation to the font size beforehand.
    pub fn from_source(source: &ImageSource, resize: Resize, area: Rect) -> Result<Self> {
        let (image, desired) = resize
            .resize(source, Rect::default(), area)
            .unwrap_or_else(|| (source.image.clone(), source.desired));
        let data = encode(&image, desired);
        Ok(Self {
            data,
            rect: desired,
        })
    }
}

fn encode(img: &DynamicImage, rect: Rect) -> Vec<HalfBlock> {
    let img = img.resize_exact(
        rect.width as u32,
        (rect.height * 2) as u32,
        FilterType::Triangle,
    );

    let mut data = vec![
        HalfBlock {
            upper: Color::Rgb(0, 0, 0),
            lower: Color::Rgb(0, 0, 0),
        };
        (rect.width * rect.height) as usize
    ];

    for (y, row) in img.to_rgb8().rows().enumerate() {
        for (x, pixel) in row.enumerate() {
            let position = x + (rect.width as usize) * (y / 2);
            if y % 2 == 0 {
                data[position].upper = Color::Rgb(pixel[0], pixel[1], pixel[2]);
            } else {
                data[position].lower = Color::Rgb(pixel[0], pixel[1], pixel[2]);
            }
        }
    }
    data
}

impl FixedBackend for FixedHalfblocks {
    fn rect(&self) -> Rect {
        self.rect
    }
    fn render(&self, area: Rect, buf: &mut Buffer) {
        for (i, hb) in self.data.iter().enumerate() {
            let x = i as u16 % self.rect.width;
            let y = i as u16 / self.rect.width;
            if x >= area.width || y >= area.height {
                continue;
            }

            buf.get_mut(area.x + x, area.y + y)
                .set_fg(hb.upper)
                .set_bg(hb.lower)
                .set_char('▀');
        }
    }
}
