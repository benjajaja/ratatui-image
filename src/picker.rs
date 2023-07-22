//! Helper module to build a backend, and swap backends at runtime
use std::path::PathBuf;

use image::{DynamicImage, Rgb};
use ratatui::layout::Rect;
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
    fixed_size: Option<Rect>,
}

#[cfg_attr(
    feature = "serde",
    derive(Deserialize),
    serde(rename_all = "lowercase")
)]
#[derive(PartialEq, Clone, Debug, Copy)]
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
    ///     Some(Rect::new(0, 0, 15, 5)),
    /// ).unwrap();
    ///
    /// // For FixedImage:
    /// let image_static = picker.new_static_fit(
    ///     dyn_img,
    ///     "./assets/Ada.png".into(),
    ///     Resize::Fit,
    /// ).unwrap();
    /// // For ResizeImage:
    /// let image_fit_state = picker.new_state();
    /// ```
    pub fn new(
        font_size: FontSize,
        backend_type: BackendType,
        background_color: Option<Rgb<u8>>,
        fixed_size: Option<Rect>,
    ) -> Result<Picker> {
        Ok(Picker {
            font_size,
            background_color,
            backend_type,
            fixed_size,
        })
    }

    /// Query the terminal window size with I/O for font size.
    #[cfg(feature = "rustix")]
    pub fn from_ioctl(
        backend_type: BackendType,
        background_color: Option<Rgb<u8>>,
        fixed_size: Option<Rect>,
    ) -> Result<Picker> {
        let stdout = rustix::stdio::stdout();
        let winsize = rustix::termios::tcgetwinsize(stdout)?;
        Picker::new(
            font_size(
                winsize.ws_xpixel,
                winsize.ws_ypixel,
                winsize.ws_col,
                winsize.ws_row,
            )?,
            backend_type,
            background_color,
            fixed_size,
        )
    }

    /// Set a specific backend
    pub fn set(&mut self, r#type: BackendType) {
        self.backend_type = r#type;
    }

    /// Returns a new *static* backend for [`crate::FixedImage`] widgets that fits into the given size.
    pub fn new_static_fit(
        &self,
        image: DynamicImage,
        path: PathBuf,
        resize: Resize,
    ) -> Result<Box<dyn FixedBackend>> {
        match self.fixed_size {
            Some(fixed_size) => {
                let source = ImageSource::new(image, path, self.font_size, self.background_color);
                match self.backend_type {
                    BackendType::Halfblocks => Ok(Box::new(FixedHalfblocks::from_source(
                        &source, resize, fixed_size,
                    )?)),
                    #[cfg(feature = "sixel")]
                    BackendType::Sixel => Ok(Box::new(FixedSixel::from_source(
                        &source, resize, fixed_size,
                    )?)),
                }
            }
            None => Err(String::from("Picker is missing fixed_size").into()),
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

fn font_size(x: u16, y: u16, cols: u16, rows: u16) -> Result<FontSize> {
    if x == 0 || y == 0 || cols == 0 || rows == 0 {
        return Err(String::from("font_size zero value").into());
    }
    Ok((x / cols, y / rows))
}

#[cfg(test)]
mod tests {
    use crate::picker::font_size;

    #[test]
    fn test_font_size() {
        assert_eq!(true, font_size(0, 0, 10, 10).is_err());
        assert_eq!(true, font_size(100, 100, 0, 0).is_err());
    }
}
