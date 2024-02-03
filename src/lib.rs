//! # Image widgets with multiple graphics protocol backends for [Ratatui]
//! [Ratatui] is an immediate-mode TUI library that does 3 things:
//!
//! 1. **Query the terminal for available graphics protocols** (or guess from `$TERM` or similar).
//! Some terminals may implement one or more graphics protocols, such as Sixels or Kitty's
//! graphics protocol. Query the terminal with some escape sequence. Fallback to "halfblocks" which
//! uses some unicode half-block characters with fore- and background colors.
//!
//! 2. **Query the terminal for the font-size in pixels.**
//! If there is an actual graphics protocol available, it is necessary to know the font-size to
//! be able to map the image pixels to character cell area. The image can be resized, fit, or
//! cropped to an area. Query the terminal for the window and columns/rows sizes, and derive the
//! font-size.
//!
//! 3. **Render the image by the means of the guessed protocol.**
//! Some protocols, like Sixels, are essentially "immediate-mode", but we still need to avoid the
//! TUI from overwriting the image area, even with blank characters.  
//!
//! Other protocols, like Kitty, are essentially stateful, but at least provide a way to re-render
//! an image that has been loaded, at a different or same position.
//!
//! # Quick start
//! ```rust
//! use ratatui::{backend::{Backend, TestBackend}, Terminal, terminal::Frame};
//! use ratatui_image::{picker::Picker, StatefulImage, protocol::StatefulProtocol};
//!
//! struct App {
//!     // We need to hold the render state.
//!     image: Box<dyn StatefulProtocol>,
//! }
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let backend = TestBackend::new(80, 30);
//!     let mut terminal = Terminal::new(backend)?;
//!
//!     // Should use Picker::from_termios(), but we can't put that here because that would break doctests!
//!     let mut picker = Picker::new((8, 12));
//!     picker.guess_protocol();
//!
//!     let dyn_img = image::io::Reader::open("./assets/Ada.png")?.decode()?;
//!     let image = picker.new_resize_protocol(dyn_img);
//!     let mut app = App { image };
//!
//!     // This would be your typical `loop {` in a real app:
//!     terminal.draw(|f| ui(f, &mut app))?;
//!
//!     Ok(())
//! }
//!
//! fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
//!     let image = StatefulImage::new(None);
//!     f.render_stateful_widget(image, f.size(), &mut app.image);
//! }
//! ```
//!
//! # Graphics protocols in terminals
//! Different terminals support different graphics protocols such as sixels,
//! kitty-graphics-protocol, or iTerm2-graphics-protocol. If no such protocol is supported, it is
//! still possible to render images with unicode "halfblocks" that have fore- and background color.
//!
//! The [picker::Picker] helper is there to do all this graphics-protocol guessing, and also to map
//! character-cell-size to pixel size so that we can e.g. "fit" an image inside a desired
//! columns+rows bound etc.
//!
//! # Widget choice
//! * The [Image] widget does not adapt to rendering area (except not drawing at all if space
//! is insufficient), may be a bit more bug prone (overdrawing or artifacts), and is not friendly
//! with some of the protocols (e.g. the Kitty graphics protocol, which is stateful). Its big
//! upside is that it is _stateless_ (in terms of ratatui, i.e. immediate-mode), and thus can never
//! block the rendering thread/task. A lot of ratatui apps only use stateless widgets.
//! * The [StatefulImage] widget adapts to its render area, is more robust against overdraw bugs and
//! artifacts, and plays nicer with some of the graphics protocols.
//! The resizing and encoding is blocking by default, but it is possible to offload this to another
//! thread or async task (see `examples/async.rs`). It must be rendered with
//! [`render_stateful_widget`] (i.e. with some mutable state).
//!
//! # Examples
//!
//! `examples/demo.rs` is a fully fledged demo:
//! * Guessing the graphics protocol and the terminal font-size.
//! * Both [Image] and [StatefulImage].
//! * [Resize::Fit] and [Resize::Crop].
//! * Reacts to resizes from terminal or widget layout.
//! * Cycle through available graphics protocols at runtime.
//! * Load different images.
//! * Cycle toggling [Image], [StatefulImage], or both, to demonstrate correct state after
//!   removal.
//! * Works with crossterm and termion backends.
//!
//! `examples/async.rs` shows how to offload resize and encoding to another thread, to avoid
//! blocking the UI thread.
//!
//! The lib also includes a binary that renders an image file.
//!
//! # Features
//! * `sixel` (default) compiles with libsixel.
//! * `rustix` (default) enables much better guessing of graphics protocols with `rustix::termios::tcgetattr`.
//! * `crossterm` or `termion` should match your ratatui backend. `termwiz` is available, but not
//! working correctly with ratatu-image.
//! * `serde` for `#[derive]`s on [picker::ProtocolType] for convenience, because it might be
//! useful to save it in some user configuration.
//! * `image-defaults` (default) just enables `image/defaults` (`image` has `default-features =
//! false`). To only support a selection of image formats and cut down dependencies, disable this
//! feature, add `image` to your crate, and enable its features/formats as desired. See
//! https://doc.rust-lang.org/cargo/reference/features.html#feature-unification.
//!
//! [Ratatui]: https://github.com/ratatui-org/ratatui
//! [Sixel]: https://en.wikipedia.org/wiki/Sixel
//! [`render_stateful_widget`]: https://docs.rs/ratatui/latest/ratatui/terminal/struct.Frame.html#method.render_stateful_widget
use std::{
    cmp::{max, min},
    error::Error,
};

use image::{imageops, DynamicImage, ImageBuffer, Rgb};
use protocol::{ImageSource, Protocol, StatefulProtocol};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{StatefulWidget, Widget},
};

pub mod picker;
pub mod protocol;
pub use image::imageops::FilterType;

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
/// # use ratatui_image::{Resize, Image, protocol::Protocol};
/// struct App {
///     image_static: Box<dyn Protocol>,
/// }
/// fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
///     let image = Image::new(app.image_static.as_ref());
///     f.render_widget(image, f.size());
/// }
/// ```
pub struct Image<'a> {
    image: &'a dyn Protocol,
}

impl<'a> Image<'a> {
    pub fn new(image: &'a dyn Protocol) -> Image<'a> {
        Image { image }
    }
}

impl<'a> Widget for Image<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        self.image.render(area, buf);
    }
}

/// Resizeable image widget that uses a [StatefulProtocol] state.
///
/// This stateful widget reacts to area resizes and resizes its image data accordingly.
///
/// ```rust
/// # use ratatui::{backend::Backend, terminal::Frame};
/// # use ratatui_image::{Resize, StatefulImage, protocol::{StatefulProtocol}};
/// struct App {
///     image_state: Box<dyn StatefulProtocol>,
/// }
/// fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
///     let image = StatefulImage::new(None).resize(Resize::Crop);
///     f.render_stateful_widget(
///         image,
///         f.size(),
///         &mut app.image_state,
///     );
/// }
/// ```
pub struct StatefulImage {
    resize: Resize,
    background_color: Option<Rgb<u8>>,
}

impl StatefulImage {
    pub fn new(background_color: Option<Rgb<u8>>) -> StatefulImage {
        StatefulImage {
            resize: Resize::Fit(None),
            background_color,
        }
    }
    pub fn resize(mut self, resize: Resize) -> StatefulImage {
        self.resize = resize;
        self
    }
}

impl StatefulWidget for StatefulImage {
    type State = Box<dyn StatefulProtocol>;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        state.resize_encode_render(&self.resize, self.background_color, area, buf)
    }
}

#[derive(Debug)]
/// Resize method
pub enum Resize {
    /// Fit to area.
    ///
    /// If the width or height is smaller than the area, the image will be resized maintaining
    /// proportions.
    ///
    /// The [FilterType] (re-exported from the [image] crate) defaults to [FilterType::Nearest].
    Fit(Option<FilterType>),
    /// Crop to area.
    ///
    /// If the width or height is smaller than the area, the image will be cropped.
    /// The behaviour is the same as using [`Image`] widget with the overhead of resizing,
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
    pub fn needs_resize(
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
        static DEFAULT_FILTER_TYPE: FilterType = FilterType::Nearest;
        match self {
            Self::Fit(filter_type) => {
                source
                    .image
                    .resize(width, height, filter_type.unwrap_or(DEFAULT_FILTER_TYPE))
            }
            Self::Crop => source.image.crop_imm(0, 0, width, height),
        }
    }

    fn needs_resize_rect(&self, desired: Rect, area: Rect) -> Rect {
        match self {
            Self::Fit(_) => {
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
        let resize = Resize::Fit(None);

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
