//! Sliced image widget and protocol wrapper.
use crate::{
    FontSize, Resize,
    errors::Errors,
    picker::{Picker, ProtocolType},
    protocol::{Protocol, halfblocks::Halfblocks, kitty::Kitty, sixel::Sixel},
};
use image::DynamicImage;
use ratatui::{
    layout::{Rect, Size},
    widgets::Widget,
};

/// An image "sliced" into rows for partially displaying, for example in vertical scrolling.
///
/// Uses a specialized [`SlicedProtocol`] with specialized operations based on the protocol.
pub struct SlicedImage<'a> {
    sliced_protocol: &'a SlicedProtocol,
    size: Size,
    position: i16,
}
impl<'a> SlicedImage<'a> {
    /// Create a sliced image that will render with the given size at the given position.
    ///
    /// The position is relative to the `area` parameter of [`SlicedImage::render`], which is
    /// either a direct argument or stems from `frame.render_widget(w, area)`.
    ///
    /// Example that renders an image as if starting at 3 lines *above* the terminal viewport:
    ///
    /// ```rust
    /// # use ratatui_image::picker::Picker;
    /// # use ratatui::layout::Size;
    /// # use ratatui_image::sliced::{SlicedProtocol, SlicedImage};
    /// # let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24))?;
    /// let picker = Picker::halfblocks(); // Note: use from_query_studio
    /// let dyn_img = image::ImageReader::open("./assets/NixOS.png")?.decode()?;
    /// let font_size = picker.font_size();
    /// // This example would render the image at its actual pixel size.
    /// let size = Size::new(
    ///     dyn_img.width().div_ceil(font_size.0 as u32) as u16,
    ///     dyn_img.height().div_ceil(font_size.1 as u32) as u16,
    /// );
    /// let sliced = SlicedProtocol::new(&picker, dyn_img, size)?;
    ///
    /// terminal.draw(|f| {
    ///     let position = -3;
    ///     f.render_widget(SlicedImage::new(&sliced, size, position), f.area());
    /// });
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// The same works for e.g. ending N lines below viewport, or within any other inner area of
    /// the TUI.
    pub fn new(sliced_protocol: &'a SlicedProtocol, size: Size, position: i16) -> SlicedImage<'a> {
        SlicedImage {
            sliced_protocol,
            size,
            position,
        }
    }
}

impl Widget for SlicedImage<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        use crate::protocol::ProtocolTrait;

        let mut image_area: Rect = self.size.into();
        image_area.x = area.x;
        image_area.y = area.y;

        match &self.sliced_protocol {
            SlicedProtocol::Kitty(kitty) => {
                let skip_line_count = if self.position < 0 {
                    image_area.height -= self.position.unsigned_abs();
                    self.position.unsigned_abs()
                } else {
                    image_area.y += self.position as u16;
                    image_area.height =
                        (area.height - self.position.unsigned_abs()).min(kitty.area().height);
                    0
                };
                if image_area.height > 0 {
                    kitty.render_with_skip(image_area, buf, skip_line_count);
                }
            }
            SlicedProtocol::Sliced(proto_slice) => {
                let range = if self.position < 0 {
                    (self.position.unsigned_abs() as usize)..proto_slice.len()
                } else {
                    image_area.y += self.position.unsigned_abs();
                    0..((area.height - self.position.unsigned_abs()) as usize)
                        .min(proto_slice.len())
                };

                let mut area = image_area;
                area.height = 1;
                for i in range {
                    proto_slice[i].render(area, buf);
                    area.y += 1;
                }
            }
            SlicedProtocol::Sixel(sixel, font_height) => {
                let skip_line_count = if self.position < 0 {
                    image_area.height -= self.position.unsigned_abs();
                    self.position.unsigned_abs()
                } else {
                    image_area.y += self.position as u16;
                    image_area.height =
                        (area.height - self.position.unsigned_abs()).min(sixel.area().height);
                    0
                };
                if image_area.height > 0 {
                    if skip_line_count == 0 && image_area.height >= sixel.area().height {
                        sixel.render(image_area, buf);
                    } else {
                        sixel.render_map(image_area, buf, |data| {
                            sixel_slice::slice(
                                data,
                                skip_line_count,
                                image_area.height,
                                *font_height,
                            )
                        });
                    }
                }
            }
            SlicedProtocol::Halfblocks(halfblocks) => {
                let skip_line_count = if self.position < 0 {
                    image_area.height -= self.position.unsigned_abs();
                    self.position.unsigned_abs()
                } else {
                    image_area.y += self.position as u16;
                    image_area.height =
                        (area.height - self.position.unsigned_abs()).min(halfblocks.area().height);
                    0
                };

                halfblocks.render_with_skip(image_area, buf, skip_line_count);
            }
        }
    }
}

/// The sliced image for [`SlicedImage`].
///
/// Contains the sliced data specialized for the protocol.
pub enum SlicedProtocol {
    /// Generic, simply a list of image slices (or rows).
    /// Not suitable for Sixel, as the foot terminal has some striding glitch. In practice, this is
    /// only used for [`crate::protocol::iterm2::Iterm2`].
    Sliced(Vec<Protocol>),
    /// Takes full advantage of the unicode-placeholder mechanism.
    Kitty(Kitty),
    /// Strips sixel "bands" at render time to display only relevant parts, since the sixel format
    /// already is row based. Not pixel accurate, but good enough. Stores font-height to match
    /// against sixel "bands" height.
    ///
    /// TODO: deconstruct at encode-time instead of render-time.
    Sixel(Sixel, u16),
    /// Renders the full image (with chafa if available) for best ASCII art results, then just
    /// renders the relevant rows.
    Halfblocks(Halfblocks),
}

impl SlicedProtocol {
    /// Create a `SlicedProtocol` for the target [`ratatui::layout::Size`].
    pub fn new(
        picker: &Picker,
        dyn_img: DynamicImage,
        size: Size,
    ) -> Result<SlicedProtocol, Errors> {
        match picker.protocol_type() {
            ProtocolType::Kitty => {
                let Protocol::Kitty(kitty) = picker.new_protocol(
                    dyn_img,
                    Rect::new(0, 0, size.width, size.height),
                    Resize::Fit(None),
                )?
                else {
                    unreachable!("ProtocolType::Kitty must produce Protocol::Kitty");
                };
                Ok(SlicedProtocol::Kitty(kitty))
            }
            ProtocolType::Sixel => {
                let Protocol::Sixel(sixel) = picker.new_protocol(
                    dyn_img,
                    Rect::new(0, 0, size.width, size.height),
                    Resize::Fit(None),
                )?
                else {
                    unreachable!("ProtocolType::Sixel must produce Protocol::Sixel");
                };
                Ok(SlicedProtocol::Sixel(sixel, picker.font_size().1))
            }
            ProtocolType::Halfblocks => {
                let Protocol::Halfblocks(halfblocks) = picker.new_protocol(
                    dyn_img,
                    Rect::new(0, 0, size.width, size.height),
                    Resize::Fit(None),
                )?
                else {
                    unreachable!("ProtocolType::Halfblocks must produce Protocol::Halfblocks");
                };
                Ok(SlicedProtocol::Halfblocks(halfblocks))
            }
            _ => {
                let (slices, image_size) = Self::slice_rows(dyn_img, &picker.font_size(), size);
                let row_count = slices.len() as u16;
                let mut row_size = image_size;
                row_size.height /= row_count;
                let rows = slices
                    .into_iter()
                    .map(|row| picker.new_protocol_raw(row, row_size))
                    .collect::<Result<Vec<Protocol>, Errors>>()?;

                Ok(SlicedProtocol::Sliced(rows))
            }
        }
    }

    /// Simply slices the DynamicImage into rows.
    ///
    /// Could work for any protocol, but:
    /// * Kitty would transmit multiple times.
    /// * Halfblocks would not render as good with chafa.
    /// * Sixel glitches in foot, would otherwise be okay.
    ///
    /// So this only is used for Iterm2.
    fn slice_rows(
        image: DynamicImage,
        font_size: &FontSize,
        size: Size,
    ) -> (Vec<DynamicImage>, Rect) {
        let image = image.resize(
            (size.width * font_size.0).into(),
            (size.height * font_size.1).into(),
            image::imageops::FilterType::Nearest,
        );

        let height = image.height();
        let width = image.width();

        let row_count = (height as f64 / font_size.1 as f64).ceil() as u16;
        let mut rows = Vec::new();

        let font_height = font_size.1 as u32;
        for i in 0..row_count {
            let y = i as u32 * font_height;
            let row_height = font_height.min(height - y);
            let cropped = image.crop_imm(0, y, width, row_height);
            rows.push(cropped);
        }

        let col_count = (width as f64 / font_size.0 as f64).ceil() as u16;
        (rows, Rect::new(0, 0, col_count, row_count))
    }
}

/// Sixel "slicing" functions
///
/// Generated with an LLM, seems to work, it's just an implementation detail.
/// Sixel data consists of some start and end data, and in between are "bands" of sixels, which are
/// six pixel columns of data. Therefore it's easy to remove some sixel bands anywhere in the
/// image, for vertical clipping.
mod sixel_slice {
    pub fn slice(data: &str, skip_line_count: u16, area_height: u16, font_height: u16) -> String {
        let skip_bands = (skip_line_count * font_height).div_ceil(6) as usize;
        let take_bands = ((area_height * font_height) / 6) as usize;

        // Rebuild CSI preamble with visible_rows instead of original row count
        let preamble = rebuild_preamble(data, area_height);

        // Strip original CSI preamble, start from ESC P
        let dcs_start = data.find("\u{1b}P").unwrap_or(0);
        let data = &data[dcs_start..];

        let header_end = find_sixel_data_start(data);
        let (header, body) = data.split_at(header_end);

        let mut bands: Vec<&str> = body.split('-').collect();
        let terminator = bands.pop().unwrap_or("");

        let available = bands.len().saturating_sub(skip_bands);
        let take_bands = take_bands.min(available);

        let sliced_bands: Vec<&str> = bands
            .iter()
            .skip(skip_bands)
            .take(take_bands)
            .copied()
            .collect();

        let mut out = String::with_capacity(data.len());
        out.push_str(&preamble);
        out.push_str(header);
        out.push_str(&sliced_bands.join("-"));
        if !sliced_bands.is_empty() {
            out.push('-');
        }
        out.push_str(terminator);
        out
    }

    fn rebuild_preamble(data: &str, rows: u16) -> String {
        // Extract the erase-width from the original preamble, e.g. `\u{1b}[34X` → 34
        // Find first occurrence of ESC [ ... X
        let width_str = data
            .find("\u{1b}[")
            .and_then(|i| {
                let rest = &data[i + 2..];
                let end = rest.find('X')?;
                Some(&rest[..end])
            })
            .unwrap_or("0");

        let mut out = String::new();
        for _ in 0..rows {
            out.push_str(&format!("\u{1b}[{width_str}X\u{1b}[1B"));
        }
        // cursor back up by rows
        out.push_str(&format!("\u{1b}[{rows}A"));
        out
    }

    fn find_sixel_data_start(data: &str) -> usize {
        let bytes = data.as_bytes();
        let mut i = 0;

        // Step 1: find ESC P
        while i + 1 < bytes.len() {
            if bytes[i] == 0x1B && bytes[i + 1] == b'P' {
                break;
            }
            i += 1;
        }

        // Step 2: skip past `q`
        while i < bytes.len() && bytes[i] != b'q' {
            i += 1;
        }
        if i < bytes.len() {
            i += 1;
        }

        // Step 3: skip raster attrs and color *definitions* only
        while i < bytes.len() {
            match bytes[i] {
                b'"' => {
                    // raster attribute line, skip to next `#` or sixel data char
                    i += 1;
                    while i < bytes.len() && bytes[i] != b'#' && !(63..=126).contains(&bytes[i]) {
                        i += 1;
                    }
                }
                b'#' => {
                    // peek ahead: is this `#digits;` (color def) or `#digits` followed by data?
                    let start = i;
                    i += 1;
                    // skip digits
                    while i < bytes.len() && bytes[i].is_ascii_digit() {
                        i += 1;
                    }
                    if i < bytes.len() && bytes[i] == b';' {
                        // it's a color definition — skip the rest of it
                        while i < bytes.len() && bytes[i] != b'#' && !(63..=126).contains(&bytes[i])
                        {
                            i += 1;
                        }
                    } else {
                        // it's a color selector in band data — rewind to the `#`, we're done
                        i = start;
                        break;
                    }
                }
                63..=126 => break, // sixel data character
                _ => i += 1,
            }
        }

        i
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn test_sixel_slice_bands() {
            // Simple data with bands separated by -
            // The slice function strips preamble, so we need ESC P in the data
            let esc = '\u{1b}';
            let bs = '\\';
            // Minimal sixel-like: ESC P q "attrs" header-bands-terminator ESC backslash
            let data = format!("{esc}Pq\"1;1;8;16#0band1-band2-band3{esc}{bs}");

            // Skip 1 row, show 1 row, font height 6 means 1 band per row
            let result = slice(&data, 1, 1, 6);
            // band1 should be skipped, band2 should be present
            assert!(!result.contains("band1"));
            assert!(result.contains("band2"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slice_rows_basic() {
        use image::RgbaImage;

        // Create a 4x4 image (4 pixels wide, 4 pixels tall)
        let mut img = RgbaImage::new(4, 4);
        for y in 0..4u32 {
            for x in 0..4u32 {
                img.put_pixel(x, y, image::Rgba([(x * 64) as u8, (y * 64) as u8, 0, 255]));
            }
        }
        let dyn_img = DynamicImage::ImageRgba8(img);

        let font_size = (1, 1); // 1x1 font means 1 row per pixel row
        let size = Size::new(4, 4);

        let (rows, image_size) = SlicedProtocol::slice_rows(dyn_img, &font_size, size);

        assert_eq!(rows.len(), 4); // 4 rows
        assert_eq!(image_size, Rect::new(0, 0, 4, 4));
        assert_eq!(rows[0].height(), 1);
        assert_eq!(rows[1].height(), 1);
        assert_eq!(rows[2].height(), 1);
        assert_eq!(rows[3].height(), 1);
    }

    #[test]
    fn test_slice_rows_font_height() {
        use image::RgbaImage;

        // Create a 4x8 image
        let mut img = RgbaImage::new(4, 8);
        for y in 0..8u32 {
            for x in 0..4u32 {
                img.put_pixel(x, y, image::Rgba([(x * 64) as u8, (y * 64) as u8, 0, 255]));
            }
        }
        let dyn_img = DynamicImage::ImageRgba8(img);

        let font_size = (1, 2); // font is 2 pixels tall
        let size = Size::new(4, 4); // 4 rows

        let (rows, image_size) = SlicedProtocol::slice_rows(dyn_img, &font_size, size);

        assert_eq!(rows.len(), 4); // 4 rows
        assert_eq!(image_size, Rect::new(0, 0, 4, 4));
        // Each row should be 2 pixels tall (font height)
        for row in &rows {
            assert_eq!(row.height(), 2);
        }
    }
}
