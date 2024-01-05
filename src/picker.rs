//! Helper module to build a protocol, and swap protocols at runtime

use image::{DynamicImage, Rgb};
use ratatui::layout::Rect;
#[cfg(all(feature = "rustix", unix))]
use rustix::termios::Winsize;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{
    protocol::{
        halfblocks::{Halfblocks, StatefulHalfblocks},
        iterm2::{FixedIterm2, Iterm2State},
        kitty::{Kitty, StatefulKitty},
        sixel::{Sixel, StatefulSixel},
        Protocol, StatefulProtocol,
    },
    FontSize, ImageSource, Resize, Result,
};

#[derive(Clone, Copy)]
pub struct Picker {
    pub font_size: FontSize,
    pub background_color: Option<Rgb<u8>>,
    pub protocol_type: ProtocolType,
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
    Sixel,
    Kitty,
    Iterm2,
}

impl ProtocolType {
    pub fn next(&self) -> ProtocolType {
        match self {
            ProtocolType::Halfblocks => ProtocolType::Sixel,
            ProtocolType::Sixel => ProtocolType::Kitty,
            ProtocolType::Kitty => ProtocolType::Iterm2,
            ProtocolType::Iterm2 => ProtocolType::Halfblocks,
        }
    }
}

/// Helper for building widgets
impl Picker {
    /// Query terminal for font-size with some escape sequence.
    ///
    /// This writes and reads from stdin momentarily.
    ///
    /// # Example
    /// ```rust
    /// use ratatui_image::picker::Picker;
    /// let mut picker = Picker::from_termios();
    /// ```
    #[cfg(all(feature = "rustix", unix))]
    pub fn from_termios() -> Result<Picker> {
        use rustix::{stdio::stdout, termios::tcgetwinsize};

        let stdout = stdout();
        let font_size = font_size(tcgetwinsize(stdout)?)?;
        Ok(Picker::new(font_size))
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
    /// let mut picker = Picker::new(user_fontsize);
    /// picker.protocol_type = user_protocol;
    /// ```
    pub fn new(font_size: FontSize) -> Picker {
        Picker {
            font_size,
            background_color: None,
            protocol_type: ProtocolType::Halfblocks,
            kitty_counter: 0,
        }
    }

    /// Guess the best protocol for the current terminal by issuing some escape sequences to
    /// stdout.
    pub fn guess_protocol(&mut self) -> ProtocolType {
        self.protocol_type = guess_protocol();
        self.protocol_type
    }

    /// Cycle through available protocols.
    pub fn cycle_protocols(&mut self) -> ProtocolType {
        self.protocol_type = self.protocol_type.next();
        self.protocol_type
    }

    /// Returns a new protocol for [`crate::Image`] widgets that fits into the given size.
    pub fn new_protocol(
        &mut self,
        image: DynamicImage,
        size: Rect,
        resize: Resize,
    ) -> Result<Box<dyn Protocol>> {
        let source = ImageSource::new(image, self.font_size);
        match self.protocol_type {
            ProtocolType::Halfblocks => Ok(Box::new(Halfblocks::from_source(
                &source,
                resize,
                self.background_color,
                size,
            )?)),
            ProtocolType::Sixel => Ok(Box::new(Sixel::from_source(
                &source,
                resize,
                self.background_color,
                size,
            )?)),
            ProtocolType::Kitty => {
                self.kitty_counter = self.kitty_counter.saturating_add(1);
                Ok(Box::new(Kitty::from_source(
                    &source,
                    resize,
                    self.background_color,
                    size,
                    self.kitty_counter,
                )?))
            }
            ProtocolType::Iterm2 => Ok(Box::new(FixedIterm2::from_source(
                &source,
                resize,
                self.background_color,
                size,
            )?)),
        }
    }

    /// Returns a new *resize* protocol for [`crate::StatefulImage`] widgets.
    pub fn new_resize_protocol(&mut self, image: DynamicImage) -> Box<dyn StatefulProtocol> {
        let source = ImageSource::new(image, self.font_size);
        match self.protocol_type {
            ProtocolType::Halfblocks => Box::new(StatefulHalfblocks::new(source)),
            ProtocolType::Sixel => Box::new(StatefulSixel::new(source)),
            ProtocolType::Kitty => {
                self.kitty_counter = self.kitty_counter.saturating_add(1);
                Box::new(StatefulKitty::new(source, self.kitty_counter))
            }
            ProtocolType::Iterm2 => Box::new(Iterm2State::new(source)),
        }
    }
}

#[cfg(all(feature = "rustix", unix))]
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

// Guess what protocol should be used, with termios stdin/out queries.
fn guess_protocol() -> ProtocolType {
    #[cfg(all(feature = "rustix", unix))]
    if let Ok(t) = check_device_attrs() {
        return t;
    }

    if let Ok(term) = std::env::var("TERM") {
        if term == "mlterm" || term == "yaft-256color" {
            return ProtocolType::Sixel;
        }
        if term.contains("kitty") {
            return ProtocolType::Kitty;
        }
    }
    if let Ok(term_program) = std::env::var("TERM_PROGRAM") {
        if term_program == "MacTerm" {
            return ProtocolType::Sixel;
        }
        if term_program.contains("iTerm") {
            return ProtocolType::Iterm2;
        }
    }
    if let Ok(lc_term) = std::env::var("LC_TERMINAL") {
        if lc_term.contains("iTerm") {
            return ProtocolType::Iterm2;
        }
    }
    ProtocolType::Halfblocks
}

#[cfg(all(feature = "rustix", unix))]
/// Check for kitty or sixel terminal support.
/// see https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h2-Sixel-Graphics
/// and https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h4-Functions-using-CSI-_-ordered-by-the-final-character-lparen-s-rparen:CSI-Ps-c.1CA3
/// and https://vt100.net/docs/vt510-rm/DA1.html
/// Tested with:
/// * patched alactritty (positive)
/// * unpatched alactritty (negative)
/// * xterm -ti vt340
/// * kitty
/// * wezterm
/// * foot
/// * konsole (kitty protocol)
/// NOTE: "tested" means that it guesses correctly, not necessarily rendering correctly.
fn check_device_attrs() -> Result<ProtocolType> {
    use rustix::termios::{LocalModes, OptionalActions};

    let stdin = rustix::stdio::stdin();
    let termios_original = rustix::termios::tcgetattr(stdin)?;
    let mut termios = termios_original.clone();
    // Disable canonical mode to read without waiting for Enter, disable echoing
    termios.local_modes &= !LocalModes::ICANON;
    termios.local_modes &= !LocalModes::ECHO;
    rustix::termios::tcsetattr(stdin, OptionalActions::Drain, &termios)?;

    rustix::io::write(
        rustix::stdio::stdout(),
        // Queries first for kitty support with `_Gi=...<ESC>\` and then for "graphics attributes"
        // (sixel) with `<ESC>[c`.
        // The query for kitty might not produce any response at all and we'd be stuck reading from
        // stdin forever. But the second query should always get some kind of response.
        // See https://sw.kovidgoyal.net/kitty/graphics-protocol/#querying-support-and-available-transmission-mediums
        b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[c",
    )?;

    let mut buf = String::new();

    loop {
        let mut charbuf = [0; 1];
        rustix::io::read(stdin, &mut charbuf)?;
        if charbuf[0] == 0 {
            continue;
        }
        buf.push(char::from(charbuf[0]));
        // TODO: The response to the first kitty query could potentially be something like `<ESC>_Gi=31;error message containing a "c"<ESC>\ and we would then not detect sixel support correctly.
        if charbuf[0] == b'c' {
            break;
        }
    }
    // Reset to previous attrs
    rustix::termios::tcsetattr(stdin, OptionalActions::Now, &termios_original)?;

    if buf.contains("_Gi=31;OK") {
        return Ok(ProtocolType::Kitty);
    }
    if buf.contains(";4;") || buf.contains("?4;") || buf.contains(";4c") || buf.contains("?4c") {
        return Ok(ProtocolType::Sixel);
    }
    Err("graphics support not detected".into())
}

#[cfg(all(test, feature = "rustix"))]
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
        let mut picker = Picker::new((1, 1));
        assert_eq!(picker.cycle_protocols(), ProtocolType::Sixel);
        assert_eq!(picker.cycle_protocols(), ProtocolType::Kitty);
        assert_eq!(picker.cycle_protocols(), ProtocolType::Iterm2);
        assert_eq!(picker.cycle_protocols(), ProtocolType::Halfblocks);
    }
}
