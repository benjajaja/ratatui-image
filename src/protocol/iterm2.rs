//! ITerm2 protocol implementation.
use base64::{engine::general_purpose, Engine};
use image::{DynamicImage, Rgba};
use ratatui::{buffer::Buffer, layout::Rect};
use std::{cmp::min, format, io::Cursor};

use crate::{errors, picker::cap_parser::Parser, FontSize, ImageSource, Resize, Result};

use super::{slice_image, ProtocolTrait, StatefulProtocolTrait};

// Fixed sixel protocol
#[derive(Clone, Default)]
pub struct Iterm2 {
    // One literal image slice per row, see below why this is necessary.
    pub data: Vec<String>,
    pub area: Rect,
    pub is_tmux: bool,
}

impl Iterm2 {
    pub fn from_source(
        source: &ImageSource,
        font_size: FontSize,
        resize: Resize,
        background_color: Rgba<u8>,
        is_tmux: bool,
        area: Rect,
    ) -> Result<Self> {
        let resized = resize.resize(
            source,
            font_size,
            Rect::default(),
            area,
            background_color,
            false,
        );
        let (image, area) = match resized {
            Some((ref image, desired)) => (image, desired),
            None => (&source.image, source.area),
        };

        let data = encode(image, font_size.1, is_tmux)?;
        Ok(Self {
            data,
            area,
            is_tmux,
        })
    }
}

// Slice the image into N rows matching the terminal font size height, so that
// we can output the `Erase in Line (EL)` for each row before the potentially
// transparent image without showing stale character artifacts.
fn encode(img: &DynamicImage, font_height: u16, is_tmux: bool) -> Result<Vec<String>> {
    let results = slice_image(img, font_height as u32)
        .into_iter()
        .flat_map(|img| {
            let mut png: Vec<u8> = vec![];
            img.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)?;

            let data = general_purpose::STANDARD.encode(&png);

            let (start, escape, end) = Parser::escape_tmux(is_tmux);
            Ok::<String, errors::Errors>(format!(
                    // Clear row from cursor on, for stale characters behind transparent images.
                "{start}{escape}[0K{escape}]1337;File=inline=1;size={};width={}px;height={}px;doNotMoveCursor=1:{}\x07{end}",
                png.len(),
                img.width(),
                img.height(),
                data,
            ))
        })
        .collect();
    Ok(results)
}

impl ProtocolTrait for Iterm2 {
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        render(self.area, &self.data, area, buf, false)
    }
}

fn render(rect: Rect, data: &[String], area: Rect, buf: &mut Buffer, overdraw: bool) {
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

    // Render each slice, see `encode()` for details.
    for (i, slice) in data.iter().enumerate() {
        let x = render_area.left();
        let y = render_area.top() + (i as u16);
        buf.cell_mut((x, y)).map(|cell| cell.set_symbol(slice));
        for x in (render_area.left() + 1)..render_area.right() {
            // Skip following columns to avoid writing over the image.
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

impl StatefulProtocolTrait for StatefulIterm2 {
    fn background_color(&self) -> Rgba<u8> {
        self.source.background_color
    }
    fn needs_resize(&mut self, resize: &Resize, area: Rect) -> Option<Rect> {
        resize.needs_resize(&self.source, self.font_size, self.current.area, area, false)
    }
    fn resize_encode(&mut self, resize: &Resize, background_color: Rgba<u8>, area: Rect) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let force = self.source.hash != self.hash;
        if let Some((img, rect)) = resize.resize(
            &self.source,
            self.font_size,
            self.current.area,
            area,
            background_color,
            force,
        ) {
            let is_tmux = self.current.is_tmux;
            match encode(&img, self.font_size.1, is_tmux) {
                Ok(data) => {
                    self.current = Iterm2 {
                        data,
                        area: rect,
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
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        render(self.current.area, &self.current.data, area, buf, true);
    }
}
