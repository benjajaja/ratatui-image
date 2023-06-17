use image::DynamicImage;
use ratatui::{buffer::Buffer, layout::Rect};
use sixel_rs::{
    encoder::{Encoder, QuickFrameBuilder},
    optflags::EncodePolicy,
    status::Error,
    sys::PixelFormat,
};
use std::{cmp::min, fs, io, path::Path};

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
        let (image, rect) = resize
            .resize(source, Rect::default(), area)
            .unwrap_or_else(|| (source.image.clone(), source.desired));

        let data = encode(&image)?;
        Ok(Self { data, rect })
    }
}

// TODO: work around this abomination! There has to be a way to get the bytes without files.
const TMP_FILE: &str = "/tmp/test_out.sixel";
// TODO: change E to sixel_rs::status::Error and map when calling
pub fn encode(img: &DynamicImage) -> Result<String> {
    let (w, h) = (img.width(), img.height());
    let bytes = img.to_rgba8().as_raw().to_vec();

    let encoder = Encoder::new().map_err(sixel_err)?;
    encoder.set_output(Path::new(TMP_FILE)).map_err(sixel_err)?;
    encoder
        .set_encode_policy(EncodePolicy::Fast)
        .map_err(sixel_err)?;
    let frame = QuickFrameBuilder::new()
        .width(w as _)
        .height(h as _)
        .format(PixelFormat::RGBA8888)
        .pixels(bytes);

    encoder.encode_bytes(frame).map_err(sixel_err)?;

    let data = fs::read_to_string(TMP_FILE)?;
    fs::remove_file(TMP_FILE)?;
    Ok(data)
}

fn sixel_err(err: Error) -> io::Error {
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
}
