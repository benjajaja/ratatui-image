use image::DynamicImage;
use ratatui::{backend::Backend, buffer::Buffer, layout::Rect, Terminal};
use sixel_rs::{
    encoder::{Encoder, QuickFrameBuilder},
    optflags::EncodePolicy,
    sys::PixelFormat,
};
use std::{cmp::min, fs, io, path::Path};

use super::{img_resize, round_size_to_cells, StaticBackend};

pub mod resizeable;

// Static sixel backend
#[derive(Clone, Default)]
pub struct StaticSixel {
    pub data: String,
    pub rect: Rect,
}

impl StaticSixel {
    pub fn from_image<B: Backend>(
        img: DynamicImage,
        terminal: &mut Terminal<B>,
    ) -> Result<StaticSixel, io::Error> {
        let font_size = terminal.backend_mut().font_size()?;
        let rect = round_size_to_cells(img.width(), img.height(), font_size);
        let img = img_resize(&img, font_size, rect);
        let data = encode(&img);
        Ok(StaticSixel { data, rect })
    }
}

// TODO: work around this abomination
const TMP_FILE: &str = "./assets/test_out.sixel";
pub fn encode(img: &DynamicImage) -> String {
    let (w, h) = (img.width(), img.height());
    let bytes = img.to_rgba8().as_raw().to_vec();

    let encoder = Encoder::new().unwrap();
    encoder.set_output(Path::new(TMP_FILE)).unwrap();
    encoder.set_encode_policy(EncodePolicy::Fast).unwrap();
    let frame = QuickFrameBuilder::new()
        .width(w as _)
        .height(h as _)
        .format(PixelFormat::RGBA8888)
        .pixels(bytes);

    encoder.encode_bytes(frame).unwrap();

    let data = fs::read_to_string(TMP_FILE).unwrap();
    fs::remove_file(TMP_FILE).unwrap();
    data
}

impl StaticBackend for StaticSixel {
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
