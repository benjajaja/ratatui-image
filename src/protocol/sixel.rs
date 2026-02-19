//! Sixel protocol implementations.
//! Uses [`icy_sixel`] to draw image pixels, if the terminal [supports] the [Sixel] protocol.
//! Needs the `sixel` feature.
//!
//! [`icy_sixel`]: https://github.com/mkrueger/icy_sixel
//! [supports]: https://arewesixelyet.com
//! [Sixel]: https://en.wikipedia.org/wiki/Sixel
use std::{cmp::min, fmt::Write};

use icy_sixel::{EncodeOptions, sixel_encode};
use image::DynamicImage;
use ratatui::{
    buffer::{Buffer, CellDiffOption},
    layout::Rect,
};

use super::{ProtocolTrait, StatefulProtocolTrait};
use crate::{Result, errors::Errors, picker::cap_parser::Parser, protocol::UNIT_WIDTH};

// Fixed sixel protocol
#[derive(Clone, Default)]
pub struct Sixel {
    pub data: String,
    pub area: Rect,
    pub is_tmux: bool,
}

impl Sixel {
    pub fn new(image: DynamicImage, area: Rect, is_tmux: bool) -> Result<Self> {
        let data = encode(&image, area, is_tmux)?;
        Ok(Self {
            data,
            area,
            is_tmux,
        })
    }
}

// TODO: change E to sixel_rs::status::Error and map when calling
fn encode(img: &DynamicImage, area: Rect, is_tmux: bool) -> Result<String> {
    let (w, h) = (img.width(), img.height());
    let img_rgba8 = img.to_rgba8();
    let bytes = img_rgba8.as_raw();
    let (start, escape, end) = Parser::escape_tmux(is_tmux);

    // Transparency needs explicit erasing of stale characters, or they stay behind the rendered
    // image due to skipping of the following characters _in the buffer_.
    // See comment in iterm2::encode about why we use ECH and movements instead of DECERA.
    // TODO: unify this with iterm2
    let width = area.width;
    let height = area.height;

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
        for _ in 0..height {
            write!(data, "{escape}[{width}X{escape}[1B").unwrap();
        }
        write!(data, "{escape}[{height}A").unwrap();
        data.push_str(escape);
        data.push_str(&sixel_data[1..]);
        data.push_str(end);
    } else {
        for _ in 0..height {
            write!(data, "{escape}[{width}X{escape}[1B").unwrap();
        }
        write!(data, "{escape}[{height}A").unwrap();
        data.push_str(&sixel_data);
    }
    Ok(data)
}

impl ProtocolTrait for Sixel {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        render(self.area, &self.data, area, buf, false)
    }

    fn area(&self) -> Rect {
        self.area
    }
}

fn render(rect: Rect, data: &str, area: Rect, buf: &mut Buffer, overdraw: bool) {
    let render_area = match render_area(rect, area, overdraw) {
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
            // Note that [ResizeProtocol] forces to ignore this early return, since it will
            // always resize itself to the area.
            return;
        }
        Some(r) => r,
    };

    if let Some(cell) = buf.cell_mut(render_area) {
        cell.set_symbol(data).set_diff_option(UNIT_WIDTH);
    }

    // Skip entire area (except first cell)
    for y in render_area.top()..render_area.bottom() {
        for x in render_area.left()..render_area.right() {
            if x == render_area.left() && y == render_area.top() {
                continue;
            }
            buf.cell_mut((x, y))
                .map(|cell| cell.set_diff_option(CellDiffOption::Skip));
        }
    }
}

fn render_area(rect: Rect, area: Rect, overdraw: bool) -> Option<Rect> {
    if overdraw {
        return Some(Rect::new(
            area.x,
            area.y,
            min(rect.width, area.width),
            min(rect.height, area.height),
        ));
    }

    if rect.width > area.width || rect.height > area.height {
        return None;
    }
    Some(Rect::new(area.x, area.y, rect.width, rect.height))
}

impl StatefulProtocolTrait for Sixel {
    fn resize_encode(&mut self, img: DynamicImage, area: Rect) -> Result<()> {
        let data = encode(&img, area, self.is_tmux)?;
        *self = Sixel {
            data,
            area,
            ..*self
        };
        Ok(())
    }
}
