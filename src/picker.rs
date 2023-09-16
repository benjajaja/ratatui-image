//! Helper module to build a backend, and swap backends at runtime

use image::{DynamicImage, Rgb};
use ratatui::layout::Rect;
#[cfg(feature = "rustix")]
use rustix::termios::Winsize;
#[cfg(all(feature = "sixel", feature = "rustix"))]
use rustix::termios::{LocalModes, OptionalActions};
#[cfg(feature = "serde")]
use serde::Deserialize;

#[cfg(feature = "sixel")]
use crate::protocol::sixel::{FixedSixel, SixelState};

use crate::{
    protocol::{
        halfblocks::{FixedHalfblocks, HalfblocksState},
        kitty::{FixedKitty, KittyState},
        Protocol, ResizeProtocol,
    },
    FontSize, ImageSource, Resize, Result,
};

#[derive(Clone, Copy)]
pub struct Picker {
    font_size: FontSize,
    background_color: Option<Rgb<u8>>,
    backend_type: BackendType,
    kitty_counter: u8,
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
    Kitty,
}

impl BackendType {
    pub fn next(&self) -> BackendType {
        match self {
            #[cfg(not(feature = "sixel"))]
            BackendType::Halfblocks => BackendType::Kitty,
            #[cfg(feature = "sixel")]
            BackendType::Halfblocks => BackendType::Sixel,
            #[cfg(feature = "sixel")]
            BackendType::Sixel => BackendType::Kitty,
            BackendType::Kitty => BackendType::Halfblocks,
        }
    }
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
    /// let mut picker = Picker::new(
    ///     (7, 14),
    ///     BackendType::Halfblocks,
    ///     None,
    /// ).unwrap();
    ///
    /// // For FixedImage:
    /// let image_static = picker.new_static_fit(
    ///     dyn_img.clone(),
    ///     Rect::new(0, 0, 15, 5),
    ///     Resize::Fit,
    /// ).unwrap();
    /// // For ResizeImage:
    /// let image_fit_state = picker.new_state(dyn_img);
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
            kitty_counter: 0,
        })
    }

    /// Query the terminal window size with I/O for font size and graphics capabilities.
    ///
    /// This writes and reads from stdin momentarily. Best be called *before* initializing the
    /// terminal backend, to be safe.
    #[cfg(feature = "rustix")]
    pub fn from_termios(background_color: Option<Rgb<u8>>) -> Result<Picker> {
        let stdout = rustix::stdio::stdout();
        let font_size = font_size(rustix::termios::tcgetwinsize(stdout)?)?;
        Picker::new(font_size, guess_backend(), background_color)
    }

    /// Set a specific backend
    pub fn set(&mut self, r#type: BackendType) {
        self.backend_type = r#type;
    }

    /// Cycle through available backends
    pub fn cycle_backends(&mut self) -> BackendType {
        self.backend_type = self.backend_type.next();
        self.backend_type
    }

    /// Returns a new *static* backend for [`crate::FixedImage`] widgets that fits into the given size.
    pub fn new_static_fit(
        &mut self,
        image: DynamicImage,
        size: Rect,
        resize: Resize,
    ) -> Result<Box<dyn Protocol>> {
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
            BackendType::Kitty => {
                self.kitty_counter += 1;
                Ok(Box::new(FixedKitty::from_source(
                    &source,
                    resize,
                    self.background_color,
                    size,
                    self.kitty_counter,
                )?))
            }
        }
    }

    /// Returns a new *state* backend for [`crate::ResizeImage`].
    pub fn new_state(&mut self, image: DynamicImage) -> Box<dyn ResizeProtocol> {
        let source = ImageSource::new(image, self.font_size);
        match self.backend_type {
            BackendType::Halfblocks => Box::new(HalfblocksState::new(source)),
            #[cfg(feature = "sixel")]
            BackendType::Sixel => Box::new(SixelState::new(source)),
            BackendType::Kitty => {
                self.kitty_counter += 1;
                Box::new(KittyState::new(source, self.kitty_counter))
            }
        }
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

#[cfg(feature = "rustix")]
// Guess what protocol should be used, with termios stdin/out queries.
fn guess_backend() -> BackendType {
    if let Ok(term) = std::env::var("TERM") {
        match term.as_str() {
            #[cfg(all(feature = "sixel", feature = "rustix"))]
            "mlterm" | "yaft-256color" => {
                return BackendType::Sixel;
            }
            term => {
                #[cfg(all(feature = "sixel", feature = "rustix"))]
                match check_device_attrs() {
                    Ok(t) => return t,
                    Err(err) => eprintln!("{err}"),
                };
                if term.contains("kitty") {
                    return BackendType::Kitty;
                }
                #[cfg(all(feature = "sixel", feature = "rustix"))]
                if let Ok(term_program) = std::env::var("TERM_PROGRAM") {
                    if term_program == "MacTerm" {
                        return BackendType::Sixel;
                    }
                }
            }
        }
    }
    BackendType::Halfblocks
}

#[cfg(all(feature = "sixel", feature = "rustix"))]
/// Check if Sixel is within the terminal's attributes
/// see https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h2-Sixel-Graphics
/// and https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h4-Functions-using-CSI-_-ordered-by-the-final-character-lparen-s-rparen:CSI-Ps-c.1CA3
/// and https://vt100.net/docs/vt510-rm/DA1.html
/// Tested with:
/// * foot
/// * patched alactritty (positive)
/// * unpatched alactritty (negative)
/// * xterm -ti vt340
/// * kitty (negative)
/// * wezterm
fn check_device_attrs() -> Result<BackendType> {
    let stdin = rustix::stdio::stdin();
    let termios_original = rustix::termios::tcgetattr(stdin)?;
    let mut termios = termios_original.clone();
    // Disable canonical mode to read without waiting for Enter, disable echoing
    termios.local_modes &= !LocalModes::ICANON;
    termios.local_modes &= !LocalModes::ECHO;
    rustix::termios::tcsetattr(stdin, OptionalActions::Drain, &termios)?;

    rustix::io::write(rustix::stdio::stdout(), b"\x1b[c")?;

    let mut buf = String::new();
    loop {
        let mut charbuf = [0; 1];
        rustix::io::read(stdin, &mut charbuf)?;
        if charbuf[0] == 0 {
            continue;
        }
        buf.push(char::from(charbuf[0]));
        if charbuf[0] == b'c' {
            break;
        }
    }
    // Reset to previous attrs
    rustix::termios::tcsetattr(stdin, OptionalActions::Now, &termios_original)?;

    if buf.contains(";4;") || buf.contains("?4;") || buf.contains(";4c") || buf.contains("?4c") {
        Ok(BackendType::Sixel)
    } else {
        Err(format!(
            "CSI sixel support not detected: ^[{}",
            if buf.len() > 1 {
                &buf[1..]
            } else {
                "(nothing)"
            }
        )
        .into())
    }
}

#[cfg(all(test, feature = "rustix", feature = "sixel"))]
mod tests {
    use std::assert_eq;

    use crate::picker::{font_size, BackendType, Picker};
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

    #[test]
    fn test_cycle_backends() {
        let mut picker = Picker::new((1, 1), BackendType::Halfblocks, None).unwrap();
        #[cfg(feature = "sixel")]
        assert_eq!(picker.cycle_backends(), BackendType::Sixel);
        assert_eq!(picker.cycle_backends(), BackendType::Kitty);
        assert_eq!(picker.cycle_backends(), BackendType::Halfblocks);
    }
}
