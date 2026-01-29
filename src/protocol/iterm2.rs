//! ITerm2 protocol implementation.
use image::DynamicImage;
use ratatui::{buffer::Buffer, layout::Rect};
use std::{cmp::min, fmt::Write, io::Cursor};

use crate::{Result, picker::cap_parser::Parser};

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

    let (start, escape, end) = Parser::escape_tmux(is_tmux);

    // Transparency needs explicit erasing of stale characters, or they stay behind the rendered
    // image due to skipping of the following characters _in the buffer_.
    // DECERA does not work in WezTerm, however ECH and and cursor CUD and CUU do.
    // For each line, erase `width` characters, then move back and place image.
    // TODO: unify this with sixel
    let width = render_area.width;
    let height = render_area.height;
    let mut seq = String::from(start);
    for _ in 0..height {
        write!(seq, "{escape}[{width}X{escape}[1B").unwrap();
    }
    write!(seq, "{escape}[{height}A").unwrap();

    write!(
        seq,
        "{escape}]1337;File=inline=1;size={};width={}px;height={}px;doNotMoveCursor=1:",
        png.len(),
        img.width(),
        img.height(),
    )
    .unwrap();

    base64_simd::STANDARD.encode_append(&png, &mut seq);

    write!(seq, "\x07{end}").unwrap();
    Ok(seq)
}

impl ProtocolTrait for Iterm2 {
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
            // Note that [StatefulProtocol] forces to ignore this early return, since it will
            // always resize itself to the area.
            return;
        }
        Some(r) => r,
    };

    buf.cell_mut(render_area).map(|cell| cell.set_symbol(data));

    for x in (render_area.left() + 1)..render_area.right() {
        if let Some(cell) = buf.cell_mut((x, render_area.top())) {
            cell.set_skip(true);
        }
    }

    // Skip entire area
    for y in (render_area.top() + 1)..render_area.bottom() {
        for x in render_area.left()..render_area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_skip(true);
            }
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

impl StatefulProtocolTrait for Iterm2 {
    fn resize_encode(&mut self, img: DynamicImage, area: Rect) -> Result<()> {
        let data = encode(&img, area, self.is_tmux)?;
        *self = Iterm2 {
            data,
            area,
            ..*self
        };
        Ok(())
    }
}
