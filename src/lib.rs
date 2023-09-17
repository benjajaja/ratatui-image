//! Image widgets for [Ratatui]
//!
//! **⚠️ THIS CRATE IS EXPERIMENTAL**
//!
//! **⚠️ THE `TERMWIZ` RATATUI BACKEND IS BROKEN WITH THIS CRATE**
//!
//! Render images with graphics protocols in the terminal with [Ratatui].
//!
//! # Quick start
//! ```rust
//! use ratatui::{backend::{Backend, TestBackend}, Terminal, terminal::Frame, layout::Rect};
//! use ratatui_image::{
//!   picker::{Picker, ProtocolType},
//!   Resize, ResizeImage, protocol::{ImageSource, ResizeProtocol},
//! };
//!
//! struct App {
//!     // We need to hold the render state.
//!     image: Box<dyn ResizeProtocol>,
//! }
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // It is highly recommended to use Picker::from_termios() instead!
//!     let mut picker = Picker::new((7, 16), ProtocolType::Sixel, None)?;
//!
//!     let dyn_img = image::io::Reader::open("./assets/Ada.png")?.decode()?;
//!     let image = picker.new_state(dyn_img);
//!     let mut app = App { image };
//!
//!     let backend = TestBackend::new(80, 30);
//!     let mut terminal = Terminal::new(backend)?;
//!
//!     // loop:
//!     terminal.draw(|f| ui(f, &mut app))?;
//!
//!     Ok(())
//! }
//!
//! fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
//!     let image = ResizeImage::new(None);
//!     f.render_stateful_widget(image, f.size(), &mut app.image);
//! }
//! ```
//!
//! # Widget choice
//! The [ResizeImage] widget adapts to its render area, is more robust against overdraw bugs and
//! artifacts, and plays nicer with some of the graphics protocols. However, frequent render area
//! resizes might affect performance.
//!
//! The [FixedImage] widgets does not adapt to rendering area (except not drawing at all if space
//! is insufficient), is more bug prone (overdrawing or artifacts), and is not aligned with some of
//! the protocols. Its only upside is that it is stateless (in terms of ratatui), and thus is not
//! performance-impacted by render area resizes.
//!
//! # Examples
//!
//! See the [crate::picker::Picker] helper and [`examples/demo`](./examples/demo/main.rs).
//! The lib also includes a binary that renders an image file.
//!
//! # Features
//! * `sixel` (default) compiles with libsixel.
//! * `rustix` (default) enables [picker::Picker::from_termios] to guess which graphics protocol to use and what
//! font-size the terminal has.
//! * `crossterm` / `termion` / `termwiz` should match your ratatui backend. `termwiz` is not
//! working correctly with ratatu-image!
//! * `serde` for `#[derive]`s on [picker::ProtocolType] for convenience, because it might be
//! useful to save it in some user configuration.
//!
//! [Ratatui]: https://github.com/ratatui-org/ratatui
//! [Sixel]: https://en.wikipedia.org/wiki/Sixel
//! [Ratatui PR for cell skipping]: https://github.com/ratatui-org/ratatui/pull/215
//! [Ratatui PR for getting window size]: https://github.com/ratatui-org/ratatui/pull/276
use std::{
    cmp::{max, min},
    error::Error,
};

use image::{
    imageops::{self, FilterType},
    DynamicImage, ImageBuffer, Rgb,
};
use protocol::{ImageSource, Protocol, ResizeProtocol};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{StatefulWidget, Widget},
};

pub mod picker;
pub mod protocol;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// The terminal's font size in `(width, height)`
pub type FontSize = (u16, u16);

/// Fixed size image widget that uses [Protocol].
///
/// The widget does **not** react to area resizes, and is not even guaranteed to **not** overdraw.
/// Its advantage lies in that the [Protocol] needs only one initial resize.
///
/// ```rust
/// # use ratatui::{backend::Backend, terminal::Frame};
/// # use ratatui_image::{Resize, FixedImage, protocol::Protocol};
/// struct App {
///     image_static: Box<dyn Protocol>,
/// }
/// fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
///     let image = FixedImage::new(app.image_static.as_ref());
///     f.render_widget(image, f.size());
/// }
/// ```
pub struct FixedImage<'a> {
    image: &'a dyn Protocol,
}

impl<'a> FixedImage<'a> {
    pub fn new(image: &'a dyn Protocol) -> FixedImage<'a> {
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

/// Resizeable image widget that uses a [ResizeProtocol] state.
///
/// This stateful widget reacts to area resizes and resizes its image data accordingly.
///
/// ```rust
/// # use ratatui::{backend::Backend, terminal::Frame};
/// # use ratatui_image::{Resize, ResizeImage, protocol::{ResizeProtocol}};
/// struct App {
///     image_state: Box<dyn ResizeProtocol>,
/// }
/// fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
///     let image = ResizeImage::new(None).resize(Resize::Crop);
///     f.render_stateful_widget(
///         image,
///         f.size(),
///         &mut app.image_state,
///     );
/// }
/// ```
pub struct ResizeImage {
    resize: Resize,
    background_color: Option<Rgb<u8>>,
}

impl ResizeImage {
    pub fn new(background_color: Option<Rgb<u8>>) -> ResizeImage {
        ResizeImage {
            resize: Resize::Fit,
            background_color,
        }
    }
    pub fn resize(mut self, resize: Resize) -> ResizeImage {
        self.resize = resize;
        self
    }
}

impl StatefulWidget for ResizeImage {
    type State = Box<dyn ResizeProtocol>;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.render(&self.resize, self.background_color, area, buf)
    }
}

#[derive(Debug)]
/// Resize method
pub enum Resize {
    /// Fit to area.
    ///
    /// If the width or height is smaller than the area, the image will be resized maintaining
    /// proportions.
    Fit,
    /// Crop to area.
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
        background_color: Option<Rgb<u8>>,
        force: bool,
    ) -> Option<(DynamicImage, Rect)> {
        self.needs_resize(source, current, area, force).map(|rect| {
            let width = (rect.width * source.font_size.0) as u32;
            let height = (rect.height * source.font_size.1) as u32;
            // Resize/Crop/etc. but not necessarily fitting cell size
            let mut image = self.resize_image(source, width, height);
            // Pad to cell size
            if image.width() != width || image.height() != height {
                static DEFAULT_BACKGROUND: Rgb<u8> = Rgb([0, 0, 0]);
                let color = background_color.unwrap_or(DEFAULT_BACKGROUND);
                let mut bg: DynamicImage = ImageBuffer::from_pixel(width, height, color).into();
                imageops::overlay(&mut bg, &image, 0, 0);
                image = bg;
            }
            (image, rect)
        })
    }

    /// Check if [`ImageSource`]'s "desired" fits into `area` and is different than `current`.
    fn needs_resize(
        &self,
        image: &ImageSource,
        current: Rect,
        area: Rect,
        force: bool,
    ) -> Option<Rect> {
        let desired = image.desired;
        // Check if resize is needed at all.
        if desired.width <= area.width && desired.height <= area.height && desired == current {
            let width = (desired.width * image.font_size.0) as u32;
            let height = (desired.height * image.font_size.1) as u32;
            if !force && (image.image.width() == width || image.image.height() == height) {
                return None;
            }
        }

        let rect = self.needs_resize_rect(desired, area);
        if force || rect != current {
            return Some(rect);
        }
        None
    }

    fn resize_image(&self, source: &ImageSource, width: u32, height: u32) -> DynamicImage {
        match self {
            Self::Fit => source.image.resize(width, height, FilterType::Nearest),
            Self::Crop => source.image.crop_imm(0, 0, width, height),
        }
    }

    fn needs_resize_rect(&self, desired: Rect, area: Rect) -> Rect {
        match self {
            Self::Fit => {
                let (width, height) = resize_pixels(
                    desired.width,
                    desired.height,
                    min(area.width, desired.width),
                    min(area.height, desired.height),
                );
                Rect::new(0, 0, width, height)
            }
            Self::Crop => Rect::new(
                0,
                0,
                min(desired.width, area.width),
                min(desired.height, area.height),
            ),
        }
    }
}

/// Ripped from https://github.com/image-rs/image/blob/master/src/math/utils.rs#L12
/// Calculates the width and height an image should be resized to.
/// This preserves aspect ratio, and based on the `fill` parameter
/// will either fill the dimensions to fit inside the smaller constraint
/// (will overflow the specified bounds on one axis to preserve
/// aspect ratio), or will shrink so that both dimensions are
/// completely contained within the given `width` and `height`,
/// with empty space on one axis.
fn resize_pixels(width: u16, height: u16, nwidth: u16, nheight: u16) -> (u16, u16) {
    let wratio = nwidth as f64 / width as f64;
    let hratio = nheight as f64 / height as f64;

    let ratio = f64::min(wratio, hratio);

    let nw = max((width as f64 * ratio).round() as u64, 1);
    let nh = max((height as f64 * ratio).round() as u64, 1);

    if nw > u64::from(u16::MAX) {
        let ratio = u16::MAX as f64 / width as f64;
        (u16::MAX, max((height as f64 * ratio).round() as u16, 1))
    } else if nh > u64::from(u16::MAX) {
        let ratio = u16::MAX as f64 / height as f64;
        (max((width as f64 * ratio).round() as u16, 1), u16::MAX)
    } else {
        (nw as u16, nh as u16)
    }
}

#[cfg(test)]
mod tests {
    use image::{ImageBuffer, Rgb};

    use super::*;

    const FONT_SIZE: FontSize = (10, 10);

    fn s(w: u16, h: u16) -> ImageSource {
        let image: DynamicImage =
            ImageBuffer::from_pixel(w as _, h as _, Rgb::<u8>([255, 0, 0])).into();
        ImageSource::new(image, FONT_SIZE)
    }

    fn r(w: u16, h: u16) -> Rect {
        Rect::new(0, 0, w, h)
    }

    #[test]
    fn needs_resize_fit() {
        let resize = Resize::Fit;

        let to = resize.needs_resize(&s(100, 100), r(10, 10), r(10, 10), false);
        assert_eq!(None, to);

        let to = resize.needs_resize(&s(101, 101), r(10, 10), r(10, 10), false);
        assert_eq!(None, to);

        let to = resize.needs_resize(&s(80, 100), r(8, 10), r(10, 10), false);
        assert_eq!(None, to);

        let to = resize.needs_resize(&s(100, 100), r(99, 99), r(8, 10), false);
        assert_eq!(Some(r(8, 8)), to);

        let to = resize.needs_resize(&s(100, 100), r(99, 99), r(10, 8), false);
        assert_eq!(Some(r(8, 8)), to);

        let to = resize.needs_resize(&s(100, 50), r(99, 99), r(4, 4), false);
        assert_eq!(Some(r(4, 2)), to);

        let to = resize.needs_resize(&s(50, 100), r(99, 99), r(4, 4), false);
        assert_eq!(Some(r(2, 4)), to);

        let to = resize.needs_resize(&s(100, 100), r(8, 8), r(11, 11), false);
        assert_eq!(Some(r(10, 10)), to);

        let to = resize.needs_resize(&s(100, 100), r(10, 10), r(11, 11), false);
        assert_eq!(None, to);
    }

    #[test]
    fn needs_resize_crop() {
        let resize = Resize::Crop;

        let to = resize.needs_resize(&s(100, 100), r(10, 10), r(10, 10), false);
        assert_eq!(None, to);

        let to = resize.needs_resize(&s(80, 100), r(8, 10), r(10, 10), false);
        assert_eq!(None, to);

        let to = resize.needs_resize(&s(100, 100), r(10, 10), r(8, 10), false);
        assert_eq!(Some(r(8, 10)), to);

        let to = resize.needs_resize(&s(100, 100), r(10, 10), r(10, 8), false);
        assert_eq!(Some(r(10, 8)), to);
    }
}
