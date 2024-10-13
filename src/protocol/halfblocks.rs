//! Halfblocks protocol implementations.
//! Uses the unicode character `▀` combined with foreground and background color. Assumes that the
//! font aspect ratio is roughly 1:2. Should work in all terminals.
use image::{imageops::FilterType, DynamicImage, Rgba};
use ratatui::{buffer::Buffer, layout::Rect, style::Color};

use super::{ProtocolTrait, StatefulProtocolTrait};
use crate::{FontSize, ImageSource, Resize, Result};

// Fixed Halfblocks protocol
#[derive(Clone, Default)]
pub struct Halfblocks {
    data: Vec<HalfBlock>,
    area: Rect,
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
        font_size: FontSize,
        resize: Resize,
        background_color: Option<Rgba<u8>>,
        area: Rect,
    ) -> Result<Self> {
        let resized = resize.resize(
            source,
            font_size,
            Rect::default(),
            area,
            background_color,
            false,
        );
        let (image, area) = match resized {
            Some((ref image, desired)) => (image, desired),
            None => (&source.image, source.area),
        };

        let data = encode(image, area);

        Ok(Self { data, area })
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

impl ProtocolTrait for Halfblocks {
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        for (i, hb) in self.data.iter().enumerate() {
            let x = i as u16 % self.area.width;
            let y = i as u16 / self.area.width;
            if x >= area.width || y >= area.height {
                continue;
            }

            buf.cell_mut((area.x + x, area.y + y))
                .map(|cell| cell.set_fg(hb.upper).set_bg(hb.lower).set_char('▀'));
        }
    }
}

#[derive(Clone)]
pub struct StatefulHalfblocks {
    source: ImageSource,
    font_size: FontSize,
    current: Halfblocks,
    hash: u64,
}

impl StatefulHalfblocks {
    pub fn new(source: ImageSource, font_size: FontSize) -> StatefulHalfblocks {
        StatefulHalfblocks {
            source,
            font_size,
            current: Halfblocks::default(),
            hash: u64::default(),
        }
    }
}

impl StatefulProtocolTrait for StatefulHalfblocks {
    fn needs_resize(&mut self, resize: &Resize, area: Rect) -> Option<Rect> {
        resize.needs_resize(&self.source, self.font_size, self.current.area, area, false)
    }
    fn resize_encode(&mut self, resize: &Resize, background_color: Option<Rgba<u8>>, area: Rect) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let force = self.source.hash != self.hash;
        if let Some((img, rect)) = resize.resize(
            &self.source,
            self.font_size,
            self.current.area,
            area,
            background_color,
            force,
        ) {
            let data = encode(&img, rect);
            let current = Halfblocks { data, area: rect };
            self.current = current;
            self.hash = self.source.hash;
        }
    }
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        Halfblocks::render(&mut self.current, area, buf);
    }
}
