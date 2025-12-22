//! Halfblocks protocol implementations.
//!
//! Uses the unicode character `▀` combined with foreground and background color. Assumes that the
//! font aspect ratio is roughly 1:2. Should work in all terminals.
//!
//! If chafa is available, uses chafa for much richer rendering than primitive halfblocks:
//! - `chafa-static`: statically linked at compile time
//! - `chafa-dyn`: dynamically linked at compile time
//! - `chafa-libload`: loaded at runtime via libloading, falls back to primitive if not found

// Ensure only one chafa feature is enabled at a time
#[cfg(all(feature = "chafa-static", feature = "chafa-dyn"))]
compile_error!("features `chafa-static` and `chafa-dyn` are mutually exclusive");
#[cfg(all(feature = "chafa-static", feature = "chafa-libload"))]
compile_error!("features `chafa-static` and `chafa-libload` are mutually exclusive");
#[cfg(all(feature = "chafa-dyn", feature = "chafa-libload"))]
compile_error!("features `chafa-dyn` and `chafa-libload` are mutually exclusive");

use image::DynamicImage;
use ratatui::{
    buffer::{Buffer, Cell},
    layout::Rect,
    style::Color,
};

use super::{ProtocolTrait, StatefulProtocolTrait};
use crate::Result;

#[cfg(feature = "chafa-static")]
#[path = "halfblocks/chafa_linked.rs"]
mod chafa;

#[cfg(feature = "chafa-dyn")]
#[path = "halfblocks/chafa_linked.rs"]
mod chafa;

#[cfg(feature = "chafa-libload")]
#[path = "halfblocks/chafa_libload.rs"]
mod chafa;

mod primitive;

/// Fixed Halfblocks protocol
#[derive(Clone, Default)]
pub struct Halfblocks {
    data: Vec<HalfBlock>,
    area: Rect,
}

#[derive(Clone, Debug)]
pub(crate) struct HalfBlock {
    pub upper: Color,
    pub lower: Color,
    pub char: char,
}

impl HalfBlock {
    fn set_cell(&self, cell: &mut Cell) {
        cell.set_fg(self.upper)
            .set_bg(self.lower)
            .set_char(self.char);
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

// chafa-static and chafa-dyn: always use chafa (no fallback needed/possible)
#[cfg(any(feature = "chafa-static", feature = "chafa-dyn"))]
fn encode(img: &DynamicImage, rect: Rect) -> Vec<HalfBlock> {
    chafa::encode(img, rect).expect("chafa is always available with compile-time linking")
}

// chafa-libload: try chafa, fallback to primitive if not available at runtime
#[cfg(feature = "chafa-libload")]
fn encode(img: &DynamicImage, rect: Rect) -> Vec<HalfBlock> {
    chafa::encode(img, rect).unwrap_or_else(|| primitive::encode(img, rect))
}

// no chafa feature: use primitive only
#[cfg(not(any(
    feature = "chafa-libload",
    feature = "chafa-dyn",
    feature = "chafa-static"
)))]
fn encode(img: &DynamicImage, rect: Rect) -> Vec<HalfBlock> {
    primitive::encode(img, rect)
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
    use image::{Rgb, RgbImage};
    use insta::assert_snapshot;
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};

    use crate::{
        Image,
        protocol::{Protocol, halfblocks::Halfblocks},
    };

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

        #[cfg(any(
            feature = "chafa-static",
            feature = "chafa-dyn",
            feature = "chafa-libload"
        ))]
        {
            assert!(super::chafa::is_available());
            assert_snapshot!("chafa", terminal.backend());
        }
        #[cfg(not(any(
            feature = "chafa-static",
            feature = "chafa-dyn",
            feature = "chafa-libload"
        )))]
        assert_snapshot!("halfblocks", terminal.backend());
    }
}
