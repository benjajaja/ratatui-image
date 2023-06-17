use ratatui::{buffer::Buffer, layout::Rect};

use crate::{
    backend::{img_crop, img_resize},
    DynamicBackend, ImageSource, Resize,
};

use super::{encode, StaticBackend, StaticSixel};

#[derive(Default, Clone)]
pub struct SixelState {
    current: StaticSixel,
}

impl DynamicBackend for SixelState {
    fn render(&mut self, source: &ImageSource, resize: &Resize, area: Rect, buf: &mut Buffer) {
        if let Some(rect) = resize.resize(source, self.current.rect, area) {
            eprintln!("resize ({resize:?})");
            let img = match resize {
                Resize::Fit => img_resize(&source.image, source.font_size, rect),
                Resize::Crop => img_crop(&source.image, source.font_size, rect),
            };
            let data = encode(&img);
            let current = StaticSixel { data, rect };
            self.current = current
        }
        StaticSixel::render(&self.current, area, buf);
    }
}
