//! Sixel protocol implementations.
//! Uses [`sixel-bytes`] to draw image pixels, if the terminal [supports] the [Sixel] protocol.
//! Needs the `sixel` feature.
//!
//! [`sixel-bytes`]: https://github.com/benjajaja/sixel-bytes
//! [supports]: https://arewesixelyet.com
//! [Sixel]: https://en.wikipedia.org/wiki/Sixel
use icy_sixel::{
    sixel_string, DiffusionMethod, MethodForLargest, MethodForRep, PixelFormat, Quality,
};
use image::{DynamicImage, Rgba};
use ratatui::{buffer::Buffer, layout::Rect};
use std::cmp::min;

use super::{ProtocolTrait, StatefulProtocolTrait};
use crate::{errors::Errors, FontSize, ImageSource, Resize, Result};

// Fixed sixel protocol
#[derive(Clone, Default)]
pub struct Sixel {
    pub data: String,
    pub area: Rect,
    pub is_tmux: bool,
}

impl Sixel {
    pub fn new(image: DynamicImage, area: Rect, is_tmux: bool) -> Result<Self> {
        let data = encode(&image, is_tmux)?;
        Ok(Self {
            data,
            area,
            is_tmux,
        })
    }
}

const TMUX_START: &str = "\x1bPtmux;";

// TODO: change E to sixel_rs::status::Error and map when calling
fn encode(img: &DynamicImage, is_tmux: bool) -> Result<String> {
    let (w, h) = (img.width(), img.height());
    let img_rgb8 = img.to_rgb8();
    let bytes = img_rgb8.as_raw();

    let data = sixel_string(
        bytes,
        w as i32,
        h as i32,
        PixelFormat::RGB888,
        DiffusionMethod::Stucki,
        MethodForLargest::Auto,
        MethodForRep::Auto,
        Quality::HIGH,
    )
    .map_err(|err| Errors::Sixel(err.to_string()))?;
    if is_tmux {
        if data.strip_prefix('\x1b').is_none() {
            return Err(Errors::Tmux("sixel string did not start with escape"));
        }

        let mut data_tmux = TMUX_START.to_string();
        for ch in data.chars() {
            if ch == '\x1b' {
                data_tmux.push('\x1b');
            }
            data_tmux.push(ch);
        }
        data_tmux.push('\x1b');
        data_tmux.push('\\');
        return Ok(data_tmux);
    }
    Ok(data)
}

impl ProtocolTrait for Sixel {
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
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

    buf.cell_mut(render_area).map(|cell| cell.set_symbol(data));
    let mut skip_first = false;

    // Skip entire area
    for y in render_area.top()..render_area.bottom() {
        for x in render_area.left()..render_area.right() {
            if !skip_first {
                skip_first = true;
                continue;
            }
            buf.cell_mut((x, y)).map(|cell| cell.set_skip(true));
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

#[derive(Clone)]
pub struct StatefulSixel {
    source: ImageSource,
    font_size: FontSize,
    current: Sixel,
    hash: u64,
}

impl StatefulSixel {
    pub fn new(source: ImageSource, font_size: FontSize, is_tmux: bool) -> StatefulSixel {
        StatefulSixel {
            source,
            font_size,
            current: Sixel {
                is_tmux,
                ..Sixel::default()
            },
            hash: u64::default(),
        }
    }
}

impl ProtocolTrait for StatefulSixel {
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        render(self.current.area, &self.current.data, area, buf, true);
    }

    fn area(&self) -> Rect {
        self.current.area
    }
}

impl StatefulProtocolTrait for StatefulSixel {
    fn background_color(&self) -> Rgba<u8> {
        self.source.background_color
    }
    fn needs_resize(&mut self, resize: &Resize, area: Rect) -> Option<Rect> {
        resize.needs_resize(
            &self.source,
            self.font_size,
            self.current.area,
            area,
            self.source.hash != self.hash,
        )
    }
    fn resize_encode(&mut self, resize: &Resize, background_color: Rgba<u8>, area: Rect) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let img = resize.resize(&self.source, self.font_size, area, background_color);
        let is_tmux = self.current.is_tmux;
        match encode(&img, is_tmux) {
            Ok(data) => {
                self.current = Sixel {
                    data,
                    area,
                    is_tmux,
                };
                self.hash = self.source.hash;
            }
            Err(_err) => {
                // TODO: save err in struct and expose in trait?
            }
        }
    }
}
