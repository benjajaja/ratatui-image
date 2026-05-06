//! Sixel protocol implementations.
//! Uses [`icy_sixel`] to draw image pixels, if the terminal [supports] the [Sixel] protocol.
//!
//! Delivers the image on each render as [Sixel]s.
//!
//! [`icy_sixel`]: https://github.com/mkrueger/icy_sixel
//! [supports]: https://arewesixelyet.com
//! [Sixel]: https://en.wikipedia.org/wiki/Sixel
use icy_sixel::{EncodeOptions, sixel_encode};
use image::DynamicImage;
use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect, Size},
};
use std::cmp::min;

use super::{ProtocolTrait, StatefulProtocolTrait, clear_area};
use crate::{Result, errors::Errors, picker::cap_parser::Parser};

#[derive(Clone, Default)]
pub struct Sixel {
    pub data: String,
    pub size: Size,
    pub is_tmux: bool,
}

impl Sixel {
    pub fn new(image: DynamicImage, size: Size, is_tmux: bool) -> Result<Self> {
        let data = encode(&image, size, is_tmux)?;
        Ok(Self {
            data,
            size,
            is_tmux,
        })
    }

    /// Experimental: render with a callback that can map the sixel data.
    ///
    /// Used in [`crate::sliced::SlicedImage`].
    pub(crate) fn render_map(&self, area: Rect, buf: &mut Buffer, slice: impl Fn(&str) -> String) {
        if self.size.width > area.width {
            return;
        }
        let render_area = Rect::new(
            area.x,
            area.y,
            min(self.size.width, area.width),
            min(self.size.height, area.height),
        );
        render(&slice(&self.data), render_area, buf)
    }
}

// TODO: change E to sixel_rs::status::Error and map when calling
fn encode(img: &DynamicImage, size: Size, is_tmux: bool) -> Result<String> {
    let (w, h) = (img.width(), img.height());
    let img_rgba8 = img.to_rgba8();
    let bytes = img_rgba8.as_raw();
    let (start, escape, end) = Parser::escape_tmux(is_tmux);

    let width = size.width;
    let height = size.height;

    let sixel_data = sixel_encode(bytes, w as usize, h as usize, &EncodeOptions::default())
        .map_err(|err| Errors::Sixel(format!("sixel encoding error: {err}")))?;

    let mut data = String::new();
    if is_tmux {
        if !sixel_data.starts_with('\x1b') {
            return Err(Errors::Tmux("sixel string did not start with escape"));
        }
        // The clear sequence must be inside the tmux passthrough since it uses
        // doubled escapes.
        data.push_str(start);
        clear_area(&mut data, escape, width, height);
        data.push_str(escape);
        data.push_str(&sixel_data[1..]);
        data.push_str(end);
    } else {
        clear_area(&mut data, escape, width, height);
        data.push_str(&sixel_data);
    }

    Ok(data)
}

impl ProtocolTrait for Sixel {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if self.size.width > area.width || self.size.height > area.height {
            return;
        }
        let render_area = Rect::new(area.x, area.y, self.size.width, self.size.height);

        render(&self.data, render_area, buf)
    }

    fn size(&self) -> Size {
        self.size
    }
}

fn render(data: &str, area: Rect, buf: &mut Buffer) {
    buf.cell_mut(Into::<Position>::into(area))
        .map(|cell| cell.set_symbol(data));
    let mut skip_first = false;

    // Skip entire area
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if !skip_first {
                skip_first = true;
                continue;
            }
            buf.cell_mut((x, y)).map(|cell| cell.set_skip(true));
        }
    }
}

impl StatefulProtocolTrait for Sixel {
    fn resize_encode(&mut self, img: DynamicImage, size: Size) -> Result<()> {
        let data = encode(&img, size, self.is_tmux)?;
        *self = Sixel {
            data,
            size,
            ..*self
        };
        Ok(())
    }
}
