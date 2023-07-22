use ratatui::{buffer::Buffer, layout::Rect};

use crate::{ImageSource, Resize, ResizeBackend};

use super::{encode, FixedBackend, FixedSixel};

#[derive(Default, Clone)]
pub struct SixelState {
    current: FixedSixel,
}

impl ResizeBackend for SixelState {
    fn render(&mut self, source: &ImageSource, resize: &Resize, area: Rect, buf: &mut Buffer) {
        if let Ok((data, rect)) = encode(source, resize, area) {
            let current = FixedSixel { data, rect };
            self.current = current
        }
        // TODO: save Err() in struct and expose in trait?
        FixedSixel::render(&self.current, area, buf);
    }
}
