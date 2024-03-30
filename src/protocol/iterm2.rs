//! ITerm2 protocol implementation.
use base64::{engine::general_purpose, Engine};
use image::{codecs::jpeg::JpegEncoder, DynamicImage, Rgb};
use ratatui::{buffer::Buffer, layout::Rect};
use std::{cmp::min, format};

use super::{Protocol, StatefulProtocol};
use crate::{ImageSource, Resize, Result};

// Fixed sixel protocol
#[derive(Clone, Default)]
pub struct FixedIterm2 {
    pub data: String,
    pub rect: Rect,
}

impl FixedIterm2 {
    pub fn from_source(
        source: &ImageSource,
        resize: Resize,
        background_color: Option<Rgb<u8>>,
        area: Rect,
    ) -> Result<Self> {
        let (img, rect) = resize
            .resize(source, Rect::default(), area, background_color, false)
            .unwrap_or_else(|| (source.image.clone(), source.desired));

        let data = encode(img)?;
        Ok(Self { data, rect })
    }
}

// TODO: change E to sixel_rs::status::Error and map when calling
pub fn encode(img: DynamicImage) -> Result<String> {
    let mut jpg = vec![];
    JpegEncoder::new_with_quality(&mut jpg, 75).encode_image(&img)?;
    let data = general_purpose::STANDARD.encode(&jpg);

    // TODO: get is_tmux flag, even though this seems to work in any case.
    let start = "\x1bPtmux;\x1b\x1b";
    let end = "\x1b\\";
    Ok(format!(
        "{start}]1337;File=inline=1;size={};width={}px;height={}px;doNotMoveCursor=1:{}\x07{end}",
        jpg.len(),
        img.width(),
        img.height(),
        data,
    ))
}

impl Protocol for FixedIterm2 {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        render(self.rect, &self.data, area, buf, false)
    }
    fn rect(&self) -> Rect {
        self.rect
    }
}

fn render(rect: Rect, data: &str, area: Rect, buf: &mut Buffer, overdraw: bool) {
    let render_area = match render_area(rect, area, overdraw) {
        None => {
            // If we render out of area, then the buffer will attempt to write regular text (or
            // possibly other sixels) over the image.
            //
            // Note that [StatefulProtocol] forces to ignore this early return, since it will
            // always resize itself to the area.
            return;
        }
        Some(r) => r,
    };

    buf.get_mut(render_area.left(), render_area.top())
        .set_symbol(data);
    let mut skip_first = false;

    // Skip entire area
    for y in render_area.top()..render_area.bottom() {
        for x in render_area.left()..render_area.right() {
            if !skip_first {
                skip_first = true;
                continue;
            }
            buf.get_mut(x, y).set_skip(true);
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
pub struct Iterm2State {
    source: ImageSource,
    current: FixedIterm2,
    hash: u64,
}

impl Iterm2State {
    pub fn new(source: ImageSource) -> Iterm2State {
        Iterm2State {
            source,
            current: FixedIterm2::default(),
            hash: u64::default(),
        }
    }
}

impl StatefulProtocol for Iterm2State {
    fn needs_resize(&mut self, resize: &Resize, area: Rect) -> Option<Rect> {
        resize.needs_resize(&self.source, self.current.rect, area, false)
    }
    fn resize_encode(&mut self, resize: &Resize, background_color: Option<Rgb<u8>>, area: Rect) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let force = self.source.hash != self.hash;
        if let Some((img, rect)) = resize.resize(
            &self.source,
            self.current.rect,
            area,
            background_color,
            force,
        ) {
            match encode(img) {
                Ok(data) => {
                    self.current = FixedIterm2 { data, rect };
                    self.hash = self.source.hash;
                }
                Err(_err) => {
                    // TODO: save err in struct and expose in trait?
                }
            }
        }
    }
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        render(self.current.rect, &self.current.data, area, buf, true);
    }
}
