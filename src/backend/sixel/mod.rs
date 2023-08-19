//! Sixel backend implementations.
//! Uses [`sixel-bytes`] to draw image pixels, if the terminal [supports] the [Sixel] protocol.
//! Needs the `sixel` feature.
//!
//! [`sixel-bytes`]: https://github.com/benjajaja/sixel-bytes
//! [supports]: https://arewesixelyet.com
//! [Sixel]: https://en.wikipedia.org/wiki/Sixel
use image::{DynamicImage, Rgb};
use ratatui::{buffer::Buffer, layout::Rect};
use sixel_bytes::{sixel_string, DiffusionMethod, PixelFormat, SixelError};
use std::io;

use super::{render_area, FixedBackend};
use crate::{ImageSource, Resize, Result};

pub mod resizeable;

// Fixed sixel backend
#[derive(Clone, Default)]
pub struct FixedSixel {
    pub data: String,
    pub rect: Rect,
    /// This flag is *only* for [self::resizeable::SixelState] to change `render_area()` behaviour.
    /// TODO: find a better solution.
    overdraw: bool,
}

impl FixedSixel {
    pub fn from_source(
        source: &ImageSource,
        resize: Resize,
        background_color: Option<Rgb<u8>>,
        area: Rect,
    ) -> Result<Self> {
        let (img, rect) = resize
            .resize(source, Rect::default(), area, background_color, false)
            .unwrap_or_else(|| (source.image.clone(), source.desired));

        let data = encode(img)?;
        Ok(Self {
            data,
            rect,
            overdraw: false,
        })
    }
}

// TODO: change E to sixel_rs::status::Error and map when calling
pub fn encode(img: DynamicImage) -> Result<String> {
    let (w, h) = (img.width(), img.height());
    let img_rgba8 = img.to_rgba8();
    let bytes = img_rgba8.as_raw();

    let data = sixel_string(
        bytes,
        w as _,
        h as _,
        PixelFormat::RGBA8888,
        DiffusionMethod::Stucki,
    )
    .map_err(sixel_err)?;
    Ok(data)
}

fn sixel_err(err: SixelError) -> io::Error {
    io::Error::new(io::ErrorKind::Other, format!("{err:?}"))
}

impl FixedBackend for FixedSixel {
    fn rect(&self) -> Rect {
        self.rect
    }
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let render_area = match render_area(self.rect, area, self.overdraw) {
            None => {
                // If we render out of area, then the buffer will attempt to write regular text (or
                // possibly other sixels) over the image.
                //
                // On some implementations (e.g. Xterm), this actually works but the image is
                // forever overwritten since we won't write out the same sixel data for the same
                // (col,row) position again (see buffer diffing).
                // Thus, when the area grows, the newly available cells will skip rendering and
                // leave artifacts instead of the image data.
                //
                // On some implementations (e.g. ???), only text with its foreground color is
                // overlayed on the image, also forever overwritten.
                //
                // On some implementations (e.g. patched Alactritty), image graphics are never
                // overwritten and simply draw over other UI elements.
                //
                // Note that [ResizeBackend] forces to ignore this early return, since it will
                // always resize itself to the area.
                return;
            }
            Some(r) => r,
        };

        buf.get_mut(render_area.left(), render_area.top())
            .set_symbol(&self.data);
        let mut skip_first = false;

        // Skip entire area
        for y in render_area.top()..render_area.bottom() {
            for x in render_area.left()..render_area.right() {
                if !skip_first {
                    skip_first = true;
                    continue;
                }
                buf.get_mut(x, y).set_skip(true);
            }
        }
    }
}
