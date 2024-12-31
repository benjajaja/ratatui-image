//! ITerm2 protocol implementation.
use base64::{engine::general_purpose, Engine};
use image::{DynamicImage, Rgba};
use ratatui::{buffer::Buffer, layout::Rect};
use std::{cmp::min, format, io::Cursor};

use crate::{errors, picker::cap_parser::Parser, FontSize, ImageSource, Resize, Result};

use super::{ProtocolTrait, StatefulProtocolTrait};

#[derive(Clone, Default)]
pub struct Iterm2 {
    pub data: String,
    pub area: Rect,
    pub is_tmux: bool,
}

impl Iterm2 {
    pub fn new(image: DynamicImage, area: Rect, is_tmux: bool) -> Result<Self> {
        let data = encode(&image, area, is_tmux)?;
        Ok(Self {
            data,
            area,
            is_tmux,
        })
    }
}

fn encode(img: &DynamicImage, render_area: Rect, is_tmux: bool) -> Result<String> {
    let mut png: Vec<u8> = vec![];
    img.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)?;

    let data = general_purpose::STANDARD.encode(&png);

    let (start, escape, end) = Parser::escape_tmux(is_tmux);

    // Transparency needs explicit erasing of stale characters, or they stay behind the rendered
    // image due to skipping of the following characters _in the buffer_.
    // DECERA does not work in WezTerm, however ECH and and cursor CUD and CUU do.
    // For each line, erase `width` characters, then move back and place image.
    let width = render_area.width;
    let height = render_area.height;
    let mut seq = String::from(start);
    for _ in 0..height {
        seq.push_str(&format!("{escape}[{width}X{escape}[1B").to_string());
    }
    seq.push_str(&format!("{escape}[{height}A").to_string());

    seq.push_str(&format!(
        "{escape}]1337;File=inline=1;size={};width={}px;height={}px;doNotMoveCursor=1:{}\x07",
        png.len(),
        img.width(),
        img.height(),
        data,
    ));
    seq.push_str(end);

    Ok::<String, errors::Errors>(seq)
}

impl ProtocolTrait for Iterm2 {
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
            // Note that [StatefulProtocol] forces to ignore this early return, since it will
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
pub struct StatefulIterm2 {
    source: ImageSource,
    font_size: FontSize,
    current: Iterm2,
    hash: u64,
}

impl StatefulIterm2 {
    pub fn new(source: ImageSource, font_size: FontSize, is_tmux: bool) -> StatefulIterm2 {
        StatefulIterm2 {
            source,
            font_size,
            current: Iterm2 {
                is_tmux,
                ..Iterm2::default()
            },
            hash: u64::default(),
        }
    }
}

impl ProtocolTrait for StatefulIterm2 {
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        render(self.current.area, &self.current.data, area, buf, true);
    }

    fn area(&self) -> Rect {
        self.current.area
    }
}

impl StatefulProtocolTrait for StatefulIterm2 {
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
        match encode(&img, area, is_tmux) {
            Ok(data) => {
                self.current = Iterm2 {
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
