//! Helper module to build a backend, and swap backends at runtime

use image::{DynamicImage, Rgb};
use ratatui::layout::Rect;
#[cfg(feature = "rustix")]
use rustix::termios::Winsize;
#[cfg(feature = "serde")]
use serde::Deserialize;

#[cfg(feature = "sixel")]
use crate::backend::sixel::{resizeable::SixelState, FixedSixel};

use crate::{
    backend::{
        halfblocks::{resizeable::HalfblocksState, FixedHalfblocks},
        FixedBackend, ResizeBackend,
    },
    FontSize, ImageSource, Resize, Result,
};

#[derive(Clone, Copy)]
pub struct Picker {
    font_size: FontSize,
    background_color: Option<Rgb<u8>>,
    backend_type: BackendType,
}

#[derive(PartialEq, Clone, Debug, Copy)]
#[cfg_attr(
    feature = "serde",
    derive(Deserialize),
    serde(rename_all = "lowercase")
)]
pub enum BackendType {
    Halfblocks,
    #[cfg(feature = "sixel")]
    Sixel,
}

/// Helper for building widgets
impl Picker {
    /// Create a picker from a given terminal [FontSize].
    ///
    /// # Example
    /// ```rust
    /// use std::io;
    /// use ratatu_image::{
    ///     picker::{BackendType, Picker},
    ///     Resize,
    /// };
    /// use ratatui::{
    ///     backend::{Backend, TestBackend},
    ///     layout::Rect,
    ///     Terminal,
    /// };
    ///
    /// let dyn_img = image::io::Reader::open("./assets/Ada.png").unwrap().decode().unwrap();
    /// let picker = Picker::new(
    ///     (7, 14),
    ///     BackendType::Halfblocks,
    ///     None,
    /// ).unwrap();
    ///
    /// // For FixedImage:
    /// let image_static = picker.new_static_fit(
    ///     dyn_img,
    ///     Rect::new(0, 0, 15, 5),
    ///     Resize::Fit,
    /// ).unwrap();
    /// // For ResizeImage:
    /// let image_fit_state = picker.new_state();
    /// ```
    pub fn new(
        font_size: FontSize,
        backend_type: BackendType,
        background_color: Option<Rgb<u8>>,
    ) -> Result<Picker> {
        Ok(Picker {
            font_size,
            background_color,
            backend_type,
        })
    }

    /// Query the terminal window size with I/O for font size.
    #[cfg(feature = "rustix")]
    pub fn from_ioctl(
        backend_type: BackendType,
        background_color: Option<Rgb<u8>>,
    ) -> Result<Picker> {
        let stdout = rustix::stdio::stdout();
        let font_size = font_size(rustix::termios::tcgetwinsize(stdout)?)?;
        Picker::new(font_size, backend_type, background_color)
    }

    /// Set a specific backend
    pub fn set(&mut self, r#type: BackendType) {
        self.backend_type = r#type;
    }

    /// Returns a new *static* backend for [`crate::FixedImage`] widgets that fits into the given size.
    pub fn new_static_fit(
        &self,
        image: DynamicImage,
        size: Rect,
        resize: Resize,
    ) -> Result<Box<dyn FixedBackend>> {
        let source = ImageSource::new(image, self.font_size);
        match self.backend_type {
            BackendType::Halfblocks => Ok(Box::new(FixedHalfblocks::from_source(
                &source,
                resize,
                self.background_color,
                size,
            )?)),
            #[cfg(feature = "sixel")]
            BackendType::Sixel => Ok(Box::new(FixedSixel::from_source(
                &source,
                resize,
                self.background_color,
                size,
            )?)),
        }
    }

    /// Returns a new *state* backend for [`crate::ResizeImage`].
    pub fn new_state(&self) -> Box<dyn ResizeBackend> {
        match self.backend_type {
            BackendType::Halfblocks => Box::<HalfblocksState>::default(),
            #[cfg(feature = "sixel")]
            BackendType::Sixel => Box::<SixelState>::default(),
        }
    }

    /// Cycles through available backends
    pub fn set_backend(&mut self, backend_type: BackendType) {
        self.backend_type = backend_type;
    }

    pub fn backend_type(&self) -> &BackendType {
        &self.backend_type
    }

    pub fn font_size(&self) -> FontSize {
        self.font_size
    }
}

#[cfg(feature = "rustix")]
pub fn font_size(winsize: Winsize) -> Result<FontSize> {
    let Winsize {
        ws_xpixel: x,
        ws_ypixel: y,
        ws_col: cols,
        ws_row: rows,
    } = winsize;
    if x == 0 || y == 0 || cols == 0 || rows == 0 {
        return Err(String::from("font_size zero value").into());
    }
    Ok((x / cols, y / rows))
}

#[cfg(all(test, feature = "rustix"))]
mod tests {
    use crate::picker::font_size;
    use rustix::termios::Winsize;

    #[test]
    fn test_font_size() {
        assert!(font_size(Winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 10,
            ws_ypixel: 10
        })
        .is_err());
        assert!(font_size(Winsize {
            ws_row: 10,
            ws_col: 10,
            ws_xpixel: 0,
            ws_ypixel: 0
        })
        .is_err());
    }
}
