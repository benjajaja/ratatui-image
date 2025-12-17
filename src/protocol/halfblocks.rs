//! Halfblocks protocol implementations.
//! Uses the unicode character `▀` combined with foreground and background color. Assumes that the
//! font aspect ratio is roughly 1:2. Should work in all terminals.
use image::{DynamicImage, imageops::FilterType};
use ratatui::{
    buffer::{Buffer, Cell},
    layout::Rect,
    style::Color,
};

use super::{ProtocolTrait, StatefulProtocolTrait};
use crate::Result;

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
    char: char,
}

const HALF_UPPER: char = '▀';
const HALF_LOWER: char = '▄';
const SPACE: char = ' ';

impl HalfBlock {
    fn set_cell(&self, cell: &mut Cell) {
        cell.set_fg(self.upper)
            .set_bg(self.lower)
            .set_char(self.char);
    }

    fn pick_side(&mut self) {
        if self.upper == self.lower {
            self.char = SPACE;
            return;
        }
        if HalfBlock::luminance(HalfBlock::rgb(self.lower))
            > HalfBlock::luminance(HalfBlock::rgb(self.upper))
        {
            std::mem::swap(&mut self.upper, &mut self.lower);
            self.char = HALF_LOWER;
        }
    }

    fn luminance((r, g, b): (u8, u8, u8)) -> u32 {
        2126 * r as u32 + 7152 * g as u32 + 722 * b as u32
    }

    fn rgb(color: Color) -> (u8, u8, u8) {
        match color {
            Color::Rgb(r, g, b) => (r, g, b),
            Color::Indexed(i) => HalfBlock::indexed_to_rgb(i),
            Color::Black => (0, 0, 0),
            Color::Red => (205, 0, 0),
            Color::Green => (0, 205, 0),
            Color::Yellow => (205, 205, 0),
            Color::Blue => (0, 0, 238),
            Color::Magenta => (205, 0, 205),
            Color::Cyan => (0, 205, 205),
            Color::Gray => (229, 229, 229),
            Color::DarkGray => (127, 127, 127),
            Color::LightRed => (255, 0, 0),
            Color::LightGreen => (0, 255, 0),
            Color::LightYellow => (255, 255, 0),
            Color::LightBlue => (92, 92, 255),
            Color::LightMagenta => (255, 0, 255),
            Color::LightCyan => (0, 255, 255),
            Color::White => (255, 255, 255),
            Color::Reset => (255, 255, 255), // assume light background, or pick a default
        }
    }

    fn indexed_to_rgb(i: u8) -> (u8, u8, u8) {
        match i {
            0..=15 => match i {
                0 => (0, 0, 0),
                1 => (205, 0, 0),
                2 => (0, 205, 0),
                3 => (205, 205, 0),
                4 => (0, 0, 238),
                5 => (205, 0, 205),
                6 => (0, 205, 205),
                7 => (229, 229, 229),
                8 => (127, 127, 127),
                9 => (255, 0, 0),
                10 => (0, 255, 0),
                11 => (255, 255, 0),
                12 => (92, 92, 255),
                13 => (255, 0, 255),
                14 => (0, 255, 255),
                15 => (255, 255, 255),
                _ => unreachable!(),
            },
            16..=231 => {
                // 6x6x6 color cube
                let i = i - 16;
                let r = (i / 36) % 6;
                let g = (i / 6) % 6;
                let b = i % 6;
                let to_val = |c: u8| if c == 0 { 0 } else { 55 + c * 40 };
                (to_val(r), to_val(g), to_val(b))
            }
            232..=255 => {
                // grayscale ramp
                let gray = 8 + (i - 232) * 10;
                (gray, gray, gray)
            }
        }
    }
}

impl Halfblocks {
    /// Create a FixedHalfblocks from an image.
    ///
    /// The "resolution" is determined by the font size of the terminal. Smaller fonts will result
    /// in more half-blocks for the same image size. To get a size independent of the font size,
    /// the image could be resized in relation to the font size beforehand.
    /// Also note that the font-size is probably just some arbitrary size with a 1:2 ratio when the
    /// protocol is Halfblocks, and not the actual font size of the terminal.
    pub fn new(image: DynamicImage, area: Rect) -> Result<Self> {
        let data = encode(&image, area);
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
            char: HALF_UPPER,
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
    for hb in &mut data {
        hb.pick_side();
    }
    data
}

impl ProtocolTrait for Halfblocks {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        for (i, hb) in self.data.iter().enumerate() {
            let x = i as u16 % self.area.width;
            let y = i as u16 / self.area.width;
            if x >= area.width || y >= area.height {
                continue;
            }

            if let Some(cell) = buf.cell_mut((area.x + x, area.y + y)) {
                hb.set_cell(cell);
            }
        }
    }
    fn area(&self) -> Rect {
        self.area
    }
}

impl StatefulProtocolTrait for Halfblocks {
    fn resize_encode(&mut self, img: DynamicImage, area: Rect) -> Result<()> {
        let data = encode(&img, area);
        *self = Halfblocks { data, area };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, Rgb, RgbImage};
    use insta::assert_snapshot;
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};

    use crate::{
        Image,
        protocol::{Protocol, halfblocks::Halfblocks},
    };

    #[test]
    fn render_checker() {
        let mut img = RgbImage::new(2, 2);
        img.put_pixel(0, 0, Rgb([255, 0, 0])); // red
        img.put_pixel(1, 0, Rgb([0, 255, 0])); // green
        img.put_pixel(0, 1, Rgb([0, 0, 255])); // blue
        img.put_pixel(1, 1, Rgb([255, 255, 0])); // yellow

        let image = DynamicImage::ImageRgb8(img);
        let area = Rect::new(0, 0, 2, 1);
        let hbs = Halfblocks::new(image, area).unwrap();

        let mut terminal = Terminal::new(TestBackend::new(2, 1)).unwrap();
        terminal
            .draw(|frame| frame.render_widget(Image::new(&Protocol::Halfblocks(hbs)), frame.area()))
            .unwrap();

        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn render_image() {
        let mut img = RgbImage::new(2, 2);
        img.put_pixel(0, 0, Rgb([255, 0, 0])); // red
        img.put_pixel(1, 0, Rgb([0, 255, 0])); // green
        img.put_pixel(0, 1, Rgb([0, 0, 255])); // blue
        img.put_pixel(1, 1, Rgb([255, 255, 0])); // yellow

        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal
            .draw(|frame| {
                let image = image::ImageReader::open("./assets/NixOS.png")
                    .unwrap()
                    .decode()
                    .unwrap();
                let area = Rect::new(0, 0, 40, 20);
                let hbs = Halfblocks::new(image, area).unwrap();
                frame.render_widget(Image::new(&Protocol::Halfblocks(hbs)), frame.area());
            })
            .unwrap();

        assert_snapshot!(terminal.backend());
    }
}
