use ratatui::{buffer::Buffer, layout::Rect};
use sixel_bytes::{sixel_string, DiffusionMethod, PixelFormat, SixelError};
use std::{cmp::min, io};

use super::FixedBackend;
use crate::{ImageSource, Resize, Result};

pub mod resizeable;

// Fixed sixel backend
#[derive(Clone, Default)]
pub struct FixedSixel {
    pub data: String,
    pub rect: Rect,
}

impl FixedSixel {
    pub fn from_source(source: &ImageSource, resize: Resize, area: Rect) -> Result<Self> {
        let (data, rect) = encode(source, &resize, area)?;
        Ok(Self { data, rect })
    }
}

// TODO: change E to sixel_rs::status::Error and map when calling
pub fn encode(source: &ImageSource, resize: &Resize, area: Rect) -> Result<(String, Rect)> {
    let (img, rect) = resize
        .resize(source, Rect::default(), area)
        .unwrap_or_else(|| (source.image.clone(), source.desired));

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
    Ok((data, rect))
}

fn sixel_err(err: SixelError) -> io::Error {
    io::Error::new(io::ErrorKind::Other, format!("{err:?}"))
}

impl FixedBackend for FixedSixel {
    fn rect(&self) -> Rect {
        self.rect
    }
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let rect = self.rect;
        let render_area = Rect::new(
            area.x,
            area.y,
            min(rect.width, area.width),
            min(rect.height, area.height),
        );

        // Skip entire area
        for y in render_area.top()..render_area.bottom() {
            for x in render_area.left()..render_area.right() {
                buf.get_mut(x, y).set_skip(Some(true));
            }
        }
        // ...except the first cell which "prints" all the sixel data.
        buf.get_mut(render_area.left(), render_area.top())
            .set_skip(Some(false))
            .set_symbol(&self.data);
    }
    fn data(&self) -> String {
        self.data.clone()
    }
}
