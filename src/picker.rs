//! Helper module to build a protocol, and swap protocols at runtime

use image::{DynamicImage, Rgb};
use ratatui::layout::Rect;
#[cfg(feature = "rustix")]
use rustix::termios::Winsize;
#[cfg(all(feature = "sixel", feature = "rustix"))]
use rustix::termios::{LocalModes, OptionalActions};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

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
    protocol_type: ProtocolType,
    kitty_counter: u8,
}

/// Serde-friendly protocol-type enum for [Picker].
#[derive(PartialEq, Clone, Debug, Copy)]
#[cfg_attr(
    feature = "serde",
    derive(Deserialize, Serialize),
    serde(rename_all = "lowercase")
)]
pub enum ProtocolType {
    Halfblocks,
    #[cfg(feature = "sixel")]
    Sixel,
    Kitty,
}

impl ProtocolType {
    pub fn next(&self) -> ProtocolType {
        match self {
            #[cfg(not(feature = "sixel"))]
            ProtocolType::Halfblocks => ProtocolType::Kitty,
            #[cfg(feature = "sixel")]
            ProtocolType::Halfblocks => ProtocolType::Sixel,
            #[cfg(feature = "sixel")]
            ProtocolType::Sixel => ProtocolType::Kitty,
            ProtocolType::Kitty => ProtocolType::Halfblocks,
        }
    }
}

/// Helper for building widgets
impl Picker {
    /// Guess both font-size and appropiate graphics protocol to use.
    ///
    /// This writes and reads from stdin momentarily. Best be called *before* initializing the
    /// terminal backend, to be safe.
    ///
    /// # Example
    /// ```rust
    /// use ratatui_image::picker::Picker;
    /// let mut picker = Picker::from_termios(None);
    /// ```
    #[cfg(feature = "rustix")]
    pub fn from_termios(background_color: Option<Rgb<u8>>) -> Result<Picker> {
        let stdout = rustix::stdio::stdout();
        let font_size = font_size(rustix::termios::tcgetwinsize(stdout)?)?;
        Picker::new(font_size, guess_protocol(), background_color)
    }

    /// Create a picker from a given terminal [FontSize] and [ProtocolType].
    /// This is useful to allow overriding the best-guess of [Picker::from_termios], for example
    /// from some user configuration.
    ///
    /// # Example
    /// ```rust
    /// use ratatui_image::picker::{ProtocolType, Picker};
    ///
    /// let user_fontsize = (7, 14);
    /// let user_protocol = ProtocolType::Halfblocks;
    ///
    /// let mut picker = Picker::new(user_fontsize, user_protocol, None).unwrap();
    /// ```
    pub fn new(
        font_size: FontSize,
        protocol_type: ProtocolType,
        background_color: Option<Rgb<u8>>,
    ) -> Result<Picker> {
        Ok(Picker {
            font_size,
            background_color,
            protocol_type,
            kitty_counter: 0,
        })
    }

    /// Set a specific protocol.
    pub fn set(&mut self, r#type: ProtocolType) {
        self.protocol_type = r#type;
    }

    /// Cycle through available protocols.
    pub fn cycle_protocols(&mut self) -> ProtocolType {
        self.protocol_type = self.protocol_type.next();
        self.protocol_type
    }

    /// Returns a new *static* protocol for [`crate::FixedImage`] widgets that fits into the given size.
    pub fn new_static_fit(
        &mut self,
        image: DynamicImage,
        size: Rect,
        resize: Resize,
    ) -> Result<Box<dyn Protocol>> {
        let source = ImageSource::new(image, self.font_size);
        match self.protocol_type {
            ProtocolType::Halfblocks => Ok(Box::new(FixedHalfblocks::from_source(
                &source,
                resize,
                self.background_color,
                size,
            )?)),
            #[cfg(feature = "sixel")]
            ProtocolType::Sixel => Ok(Box::new(FixedSixel::from_source(
                &source,
                resize,
                self.background_color,
                size,
            )?)),
            ProtocolType::Kitty => {
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

    /// Returns a new *state* protocol for [`crate::ResizeImage`].
    pub fn new_state(&mut self, image: DynamicImage) -> Box<dyn ResizeProtocol> {
        let source = ImageSource::new(image, self.font_size);
        match self.protocol_type {
            ProtocolType::Halfblocks => Box::new(HalfblocksState::new(source)),
            #[cfg(feature = "sixel")]
            ProtocolType::Sixel => Box::new(SixelState::new(source)),
            ProtocolType::Kitty => {
                self.kitty_counter += 1;
                Box::new(KittyState::new(source, self.kitty_counter))
            }
        }
    }

    pub fn protocol_type(&self) -> &ProtocolType {
        &self.protocol_type
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
fn guess_protocol() -> ProtocolType {
    if let Ok(term) = std::env::var("TERM") {
        match term.as_str() {
            #[cfg(all(feature = "sixel", feature = "rustix"))]
            "mlterm" | "yaft-256color" => {
                return ProtocolType::Sixel;
            }
            term => {
                #[cfg(all(feature = "sixel", feature = "rustix"))]
                match check_device_attrs() {
                    Ok(t) => return t,
                    Err(err) => eprintln!("{err}"),
                };
                if term.contains("kitty") {
                    return ProtocolType::Kitty;
                }
                #[cfg(all(feature = "sixel", feature = "rustix"))]
                if let Ok(term_program) = std::env::var("TERM_PROGRAM") {
                    if term_program == "MacTerm" {
                        return ProtocolType::Sixel;
                    }
                }
            }
        }
    }
    ProtocolType::Halfblocks
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
fn check_device_attrs() -> Result<ProtocolType> {
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
        Ok(ProtocolType::Sixel)
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

    use crate::picker::{font_size, Picker, ProtocolType};
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
    fn test_cycle_protocol() {
        let mut picker = Picker::new((1, 1), ProtocolType::Halfblocks, None).unwrap();
        #[cfg(feature = "sixel")]
        assert_eq!(picker.cycle_protocols(), ProtocolType::Sixel);
        assert_eq!(picker.cycle_protocols(), ProtocolType::Kitty);
        assert_eq!(picker.cycle_protocols(), ProtocolType::Halfblocks);
    }
}
