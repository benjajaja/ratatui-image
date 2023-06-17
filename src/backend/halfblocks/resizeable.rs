use ratatui::{buffer::Buffer, layout::Rect};

use crate::{ImageSource, Resize, ResizeBackend};

use super::{encode, FixedBackend, FixedHalfblocks};

#[derive(Default, Clone)]
pub struct HalfblocksState {
    current: FixedHalfblocks,
}

impl ResizeBackend for HalfblocksState {
    fn render(&mut self, source: &ImageSource, resize: &Resize, area: Rect, buf: &mut Buffer) {
        if let Some((img, rect)) = resize.resize(source, self.current.rect, area) {
            let data = encode(&img, rect);
            let current = FixedHalfblocks { data, rect };
            self.current = current
        }
        FixedHalfblocks::render(&self.current, area, buf);
    }
}
