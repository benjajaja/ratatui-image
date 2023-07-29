use image::Rgb;
use ratatui::{buffer::Buffer, layout::Rect};

use crate::{ImageSource, Resize, ResizeBackend};

use super::{encode, FixedBackend, FixedSixel};

#[derive(Default, Clone)]
pub struct SixelState {
    current: FixedSixel,
    hash: u64,
}

impl ResizeBackend for SixelState {
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
        let force = source.hash != self.hash;
        if let Some((img, rect)) =
            resize.resize(source, self.current.rect, area, background_color, force)
        {
            match encode(img) {
                Ok(data) => {
                    let current = FixedSixel {
                        data,
                        rect,
                        overdraw: true,
                    };
                    self.current = current;
                    self.hash = source.hash;
                }
                Err(_err) => {
                    // TODO: save err in struct and expose in trait?
                }
            }
        }

        FixedSixel::render(&self.current, area, buf);
    }
}
