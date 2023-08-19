use image::Rgb;
use ratatui::{buffer::Buffer, layout::Rect};

use crate::{ImageSource, Resize, ResizeBackend};

use super::{encode, FixedBackend, FixedHalfblocks};

#[derive(Default, Clone)]
pub struct HalfblocksState {
    current: FixedHalfblocks,
    hash: u64,
}

impl ResizeBackend for HalfblocksState {
    fn rect(&self) -> Rect {
        self.current.rect()
    }
    fn render(
        &mut self,
        source: &ImageSource,
        resize: &Resize,
        background_color: Option<Rgb<u8>>,
        area: Rect,
        buf: &mut Buffer,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let force = source.hash != self.hash;
        if let Some((img, rect)) =
            resize.resize(source, self.current.rect, area, background_color, force)
        {
            let data = encode(&img, rect);
            let current = FixedHalfblocks { data, rect };
            self.current = current;
            self.hash = source.hash;
        }
        FixedHalfblocks::render(&self.current, area, buf);
    }
}
