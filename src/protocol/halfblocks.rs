//! Halfblocks protocol implementations.
//! Uses the unicode character `▀` combined with foreground and background color. Assumes that the
//! font aspect ratio is roughly 1:2. Should work in all terminals.
use image::{imageops::FilterType, DynamicImage, Rgb};
use ratatui::{buffer::Buffer, layout::Rect, style::Color};

use super::{Protocol, StatefulProtocol};
use crate::{ImageSource, Resize, Result};

// Fixed Halfblocks protocol
#[derive(Clone, Default)]
pub struct Halfblocks {
    data: Vec<HalfBlock>,
    rect: Rect,
}

#[derive(Clone, Debug)]
struct HalfBlock {
    upper: Color,
    lower: Color,
}

impl Halfblocks {
    /// Create a FixedHalfblocks from an image.
    ///
    /// The "resolution" is determined by the font size of the terminal. Smaller fonts will result
    /// in more half-blocks for the same image size. To get a size independent of the font size,
    /// the image could be resized in relation to the font size beforehand.
    pub fn from_source(
        source: &ImageSource,
        resize: Resize,
        background_color: Option<Rgb<u8>>,
        area: Rect,
    ) -> Result<Self> {
        let (image, desired) = resize
            .resize(source, Rect::default(), area, background_color, false)
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

impl Protocol for Halfblocks {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        for (i, hb) in self.data.iter().enumerate() {
            let x = i as u16 % self.rect.width;
            let y = i as u16 / self.rect.width;
            if x >= area.width || y >= area.height {
                continue;
            }

            buf.cell_mut((area.x + x, area.y + y))
                .map(|cell| cell.set_fg(hb.upper).set_bg(hb.lower).set_char('▀'));
        }
    }

    fn rect(&self) -> Rect {
        self.rect
    }
}

#[derive(Clone)]
pub struct StatefulHalfblocks {
    source: ImageSource,
    current: Halfblocks,
    hash: u64,
}

impl StatefulHalfblocks {
    pub fn new(source: ImageSource) -> StatefulHalfblocks {
        StatefulHalfblocks {
            source,
            current: Halfblocks::default(),
            hash: u64::default(),
        }
    }
}

impl StatefulProtocol for StatefulHalfblocks {
    fn needs_resize(&mut self, resize: &Resize, area: Rect) -> Option<Rect> {
        resize.needs_resize(&self.source, self.current.rect, area, false)
    }
    fn resize_encode(&mut self, resize: &Resize, background_color: Option<Rgb<u8>>, area: Rect) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let force = self.source.hash != self.hash;
        if let Some((img, rect)) = resize.resize(
            &self.source,
            self.current.rect,
            area,
            background_color,
            force,
        ) {
            let data = encode(&img, rect);
            let current = Halfblocks { data, rect };
            self.current = current;
            self.hash = self.source.hash;
        }
    }
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        Halfblocks::render(&self.current, area, buf);
    }
}
