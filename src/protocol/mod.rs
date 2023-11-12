//! Protocol backends for the widgets

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use dyn_clone::DynClone;
use image::{DynamicImage, Rgb};
use ratatui::{buffer::Buffer, layout::Rect};

use crate::FontSize;

use super::Resize;

pub mod halfblocks;
pub mod kitty;
#[cfg(feature = "sixel")]
pub mod sixel;

/// A fixed image protocol for the [crate::FixedImage] widget.
pub trait Protocol: Send + Sync {
    /// Render the currently resized and encoded data to the buffer.
    fn render(&self, area: Rect, buf: &mut Buffer);
    /// Get the [ratatui::layout::Rect] of the image.
    fn rect(&self) -> Rect;
}

/// A resizing image protocol for the [crate::ResizeImage] widget.
pub trait ResizeProtocol: Send + Sync + DynClone {
    /// Resize and encode if necessary, and render immediately.
    ///
    /// This blocks the UI thread but requires neither threads nor async.
    fn resize_encode_render(
        &mut self,
        resize: &Resize,
        background_color: Option<Rgb<u8>>,
        area: Rect,
        buf: &mut Buffer,
    ) {
        if let Some(rect) = self.needs_resize(resize, area) {
            self.resize_encode(resize, background_color, rect);
        }
        self.render(area, buf);
    }

    /// Check if the current image state would need resizing (grow or shrink) for the given area.
    ///
    /// This can be called by the UI thread to check if this [ResizeProtocol] should be sent off to
    /// some background thread/task to do the resizing and encoding, instead of rendering. The
    /// thread should then return the [ResizeProtocol] so that it can be rendered.
    fn needs_resize(&mut self, resize: &Resize, area: Rect) -> Option<Rect>;

    /// Resize the image and encode it for rendering.
    ///
    /// This can be done in a background thread, and the result is stored in this [ResizeProtocol].
    fn resize_encode(&mut self, resize: &Resize, background_color: Option<Rgb<u8>>, area: Rect);

    /// Render the currently resized and encoded data to the buffer.
    fn render(&mut self, area: Rect, buf: &mut Buffer);
}

dyn_clone::clone_trait_object!(ResizeProtocol);

#[derive(Clone)]
/// Image source for [crate::protocol::ResizeProtocol]s
///
/// A `[ResizeProtocol]` needs to resize the ImageSource to its state when the available area
/// changes. A `[Protocol]` only needs it once.
///
/// # Examples
/// ```text
/// use image::{DynamicImage, ImageBuffer, Rgb};
/// use ratatui_image::ImageSource;
///
/// let image: ImageBuffer::from_pixel(300, 200, Rgb::<u8>([255, 0, 0])).into();
/// let source = ImageSource::new(image, "filename.png", (7, 14));
/// assert_eq!((43, 14), (source.rect.width, source.rect.height));
/// ```
///
pub struct ImageSource {
    /// The original image without resizing.
    pub image: DynamicImage,
    /// The font size of the terminal.
    pub font_size: FontSize,
    /// The area that the [`ImageSource::image`] covers, but not necessarily fills.
    pub desired: Rect,
    /// TODO: document this; when image changes but it doesn't need a resize, force a render.
    pub hash: u64,
}

impl ImageSource {
    /// Create a new image source
    pub fn new(image: DynamicImage, font_size: FontSize) -> ImageSource {
        let desired =
            ImageSource::round_pixel_size_to_cells(image.width(), image.height(), font_size);

        let mut state = DefaultHasher::new();
        image.as_bytes().hash(&mut state);
        let hash = state.finish();

        ImageSource {
            image,
            font_size,
            desired,
            hash,
        }
    }
    /// Round an image pixel size to the nearest matching cell size, given a font size.
    fn round_pixel_size_to_cells(
        img_width: u32,
        img_height: u32,
        (char_width, char_height): FontSize,
    ) -> Rect {
        let width = (img_width as f32 / char_width as f32).ceil() as u16;
        let height = (img_height as f32 / char_height as f32).ceil() as u16;
        Rect::new(0, 0, width, height)
    }
}
