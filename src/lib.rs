//! Ratatui image widgets
//!
//! Render images with supported graphics protocols in the terminal with ratatui.
//! While this generally might seem *contra natura* and something fragile, it can be worthwhile in
//! some applications.
//!
//! The images are always resized so that they fit their nearest rectangle in columns/rows.
//! The reason for this is because the image shall be drawn in the same "render pass" as all
//! surrounding text, and cells under the area of the image skip the draw on the ratatui buffer
//! level, so there is no way to "clear" previous drawn text. This would leave artifacts around the
//! image border.
//! For this resizing it is necessary to query the terminal font size in width/height.
//!
//! The [`FixedImage`] widget does not react to area resizes other than not overdrawing. Note that
//! some image protocols or their implementations might not behave correctly in this aspect and
//! overdraw or flicker outside of the image area.
//!
//! The [`ResizeImage`] stateful widget does react to area size changes by either resizing or
//! cropping itself. The state consists of the latest resized image. A resize (and encode) happens
//! every time the available area changes and either the image must be shrunk or it can grow. Thus,
//! this widget may have a much bigger performance impact.
//!
//! Each widget is backed by a "backend" implementation of a given image protocol.
//!
//! Currently supported backends/protocols:
//!
//! ## Halfblocks
//! Uses the unicode character `▀` combined with foreground and background color. Assumes that the
//! font aspect ratio is roughly 1:2. Should work in all terminals.
//! ## Sixel
//! Experimental: uses temporary files.
//! Uses [`sixel-rs`] to draw image pixels, if the terminal [supports] the [Sixel] protocol.
//!
//! [`sixel-rs`]: https://github.com/orhun/sixel-rs
//! [supports]: https://arewesixelyet.com
//! [Sixel]: https://en.wikipedia.org/wiki/Sixel
//!
//! # Examples
//!
//! For a more streamlined experience, see the [`crate::picker::Picker`] helper.
//!
//! ```rust
//! use image::{DynamicImage, ImageBuffer, Rgb};
//! use ratatui_imagine::{
//!     backend::{
//!         FixedBackend,
//!         halfblocks::FixedHalfblocks,
//!     },
//!     FixedImage, ImageSource, Resize,
//! };
//!
//! let image: DynamicImage = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(300, 200).into();
//!
//! let source = ImageSource::new(image, (7, 14), None);
//!
//! let static_image = Box::new(FixedHalfblocks::from_source(
//!     &source,
//!     Resize::Fit,
//!     source.desired,
//! )).unwrap();
//! assert_eq!(43, static_image.rect().width);
//! assert_eq!(15, static_image.rect().height);
//!
//! let image_widget = FixedImage::new(&static_image);
//! ```

use std::{cmp::min, error::Error};

use backend::{FixedBackend, ResizeBackend};
use image::{
    imageops::{self, FilterType},
    DynamicImage, ImageBuffer, Rgb,
};
use ratatui::{
    backend::Backend,
    buffer::Buffer,
    layout::Rect,
    widgets::{StatefulWidget, Widget},
    Terminal,
};

pub mod backend;
pub mod picker;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Clone)]
/// Image source for backends
///
/// A `[ResizeBackend]` needs to resize the ImageSource to its state when the available area
/// changes. A `[FixedBackend]` only needs it once.
///
/// # Examples
/// ```text
/// use image::{DynamicImage, ImageBuffer, Rgb};
/// use ratatui_imagine::ImageSource;
///
/// let image: ImageBuffer::from_pixel(300, 200, Rgb::<u8>([255, 0, 0])).into();
/// let source = ImageSource::new(image, (7, 14));
/// assert_eq!((43, 14), (source.rect.width, source.rect.height));
/// ```
///
pub struct ImageSource {
    /// The original image without resizing
    pub image: DynamicImage,
    /// The font size of the terminal
    pub font_size: (u16, u16),
    /// The area that the [`ImageSource::image`] covers, but not necessarily fills
    pub desired: Rect,
    /// The background color to fill when resizing
    pub background_color: Option<Rgb<u8>>,
}

impl ImageSource {
    /// Create a new image source
    pub fn new(
        image: DynamicImage,
        font_size: (u16, u16),
        background_color: Option<Rgb<u8>>,
    ) -> ImageSource {
        let desired = round_pixel_size_to_cells(image.width(), image.height(), font_size);
        ImageSource {
            image,
            font_size,
            desired,
            background_color,
        }
    }
    /// Create a new image source from a [Terminal](ratatui::Terminal)
    pub fn with_terminal<B: Backend>(
        image: DynamicImage,
        terminal: &mut Terminal<B>,
        background_color: Option<Rgb<u8>>,
    ) -> Result<ImageSource> {
        let font_size = terminal.backend_mut().font_size()?;
        Ok(ImageSource::new(image, font_size, background_color))
    }
}

/// Round an image pixel size to the nearest matching cell size, given a font size.
fn round_pixel_size_to_cells(
    img_width: u32,
    img_height: u32,
    (char_width, char_height): (u16, u16),
) -> Rect {
    let width = (img_width as f32 / char_width as f32).ceil() as u16;
    let height = (img_height as f32 / char_height as f32).ceil() as u16;
    Rect::new(0, 0, width, height)
}

/// Fixed size image widget
pub struct FixedImage<'a> {
    image: &'a dyn FixedBackend,
}

impl<'a> FixedImage<'a> {
    pub fn new(image: &'a dyn FixedBackend) -> FixedImage<'a> {
        FixedImage { image }
    }
}

impl<'a> Widget for FixedImage<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        self.image.render(area, buf);
    }
}

/// Resizeable image widget
pub struct ResizeImage<'a> {
    source: &'a ImageSource,
    resize: Resize,
}

impl<'a> ResizeImage<'a> {
    pub fn new(source: &'a ImageSource) -> ResizeImage<'a> {
        ResizeImage {
            source,
            resize: Resize::Fit,
        }
    }
    pub fn resize(mut self, resize: Resize) -> ResizeImage<'a> {
        self.resize = resize;
        self
    }
}

impl<'a> StatefulWidget for ResizeImage<'a> {
    type State = Box<dyn ResizeBackend>;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        state.render(self.source, &self.resize, area, buf)
    }
}

#[derive(Debug)]
/// Resize method
pub enum Resize {
    /// Fit to area
    ///
    /// If the width or height is smaller than the area, the image will be resized maintaining
    /// proportions.
    Fit,
    /// Crop to area
    ///
    /// If the width or height is smaller than the area, the image will be cropped.
    /// The behaviour is the same as using [`FixedImage`] widget with the overhead of resizing,
    /// but some terminals might misbehave when overdrawing characters over graphics.
    /// For example, the sixel branch of Alacritty never draws text over a cell that is currently
    /// being rendered by some sixel sequence, not necessarily originating from the same cell.
    Crop,
}

impl Resize {
    /// Resize if [`ImageSource`]'s "desired" doesn't fit into `area`, or is different than `current`
    fn resize(
        &self,
        source: &ImageSource,
        current: Rect,
        area: Rect,
    ) -> Option<(DynamicImage, Rect)> {
        self.needs_resize(source, current, area).map(|rect| {
            let width = (rect.width * source.font_size.0) as u32;
            let height = (rect.height * source.font_size.1) as u32;
            // Resize/Crop/etc. but not necessarily fitting cell size
            let (mut image, rect) = match self {
                Resize::Fit => {
                    let image = source.image.resize(width, height, FilterType::Nearest);
                    (image, rect)
                }
                Resize::Crop => {
                    let image = source.image.crop_imm(0, 0, width, height);
                    (image, rect)
                }
            };
            // Pad to cell size
            if image.width() != width || image.height() != height {
                static DEFAULT_BACKGROUND: Rgb<u8> = Rgb([0, 0, 0]);
                let color = source.background_color.unwrap_or(DEFAULT_BACKGROUND);
                let mut bg: DynamicImage = ImageBuffer::from_pixel(width, height, color).into();
                imageops::overlay(&mut bg, &image, 0, 0);
                image = bg;
            }
            (image, rect)
        })
    }

    /// Check if [`ImageSource`]'s "desired" fits into `area` and is different than `current`.
    fn needs_resize(&self, source: &ImageSource, current: Rect, area: Rect) -> Option<Rect> {
        match self {
            Self::Fit => {
                let desired = source.desired;
                if desired.width <= area.width
                    && desired.height <= area.height
                    && desired == current
                {
                    let width = (desired.width * source.font_size.0) as u32;
                    let height = (desired.height * source.font_size.1) as u32;
                    if source.image.width() == width || source.image.height() == height {
                        return None;
                    }
                    return Some(desired);
                }
                let mut resized = desired;
                if desired.width > area.width {
                    resized.width = area.width;
                    resized.height = ((desired.height as f32)
                        * (area.width as f32 / desired.width as f32))
                        .round() as u16;
                } else if desired.height > area.height {
                    resized.height = area.height;
                    resized.width = ((desired.width as f32)
                        * (area.height as f32 / desired.height as f32))
                        .round() as u16;
                }
                Some(resized)
            }
            Self::Crop => {
                let desired = source.desired;
                if desired.width <= area.width
                    && desired.height <= area.height
                    && desired == current
                {
                    let width = (desired.width * source.font_size.0) as u32;
                    let height = (desired.height * source.font_size.1) as u32;
                    if source.image.width() == width || source.image.height() == height {
                        return None;
                    }
                    return Some(desired);
                }

                Some(Rect::new(
                    0,
                    0,
                    min(desired.width, area.width),
                    min(desired.height, area.height),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use image::{ImageBuffer, Rgb};

    use super::*;

    fn s(w: u16, h: u16, font_size: (u16, u16)) -> ImageSource {
        let image: DynamicImage =
            ImageBuffer::from_pixel(w as _, h as _, Rgb::<u8>([255, 0, 0])).into();
        ImageSource::new(image, font_size, None)
    }

    fn r(w: u16, h: u16) -> Rect {
        Rect::new(0, 0, w, h)
    }

    #[test]
    fn needs_resize_fit() {
        let resize = Resize::Fit;

        let to = resize.needs_resize(&s(100, 100, (10, 10)), r(10, 10), r(10, 10));
        assert_eq!(None, to);

        let to = resize.needs_resize(&s(80, 100, (10, 10)), r(8, 10), r(10, 10));
        assert_eq!(None, to);

        let to = resize.needs_resize(&s(100, 100, (10, 10)), r(10, 10), r(8, 10));
        assert_eq!(Some(r(8, 8)), to);

        let to = resize.needs_resize(&s(100, 100, (10, 10)), r(10, 10), r(10, 8));
        assert_eq!(Some(r(8, 8)), to);
    }

    #[test]
    fn needs_resize_crop() {
        let resize = Resize::Crop;

        let to = resize.needs_resize(&s(100, 100, (10, 10)), r(10, 10), r(10, 10));
        assert_eq!(None, to);

        let to = resize.needs_resize(&s(80, 100, (10, 10)), r(8, 10), r(10, 10));
        assert_eq!(None, to);

        let to = resize.needs_resize(&s(100, 100, (10, 10)), r(10, 10), r(8, 10));
        assert_eq!(Some(r(8, 10)), to);

        let to = resize.needs_resize(&s(100, 100, (10, 10)), r(10, 10), r(10, 8));
        assert_eq!(Some(r(10, 8)), to);
    }
}
