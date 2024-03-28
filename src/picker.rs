//! Helper module to build a protocol, and swap protocols at runtime

use std::{env, io, time::Instant};

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
        if term_program.contains("iTerm") || term_program.contains("WezTerm") {
            return ProtocolType::Iterm2;
        }
    }
    if let Ok(lc_term) = std::env::var("LC_TERMINAL") {
        if lc_term.contains("iTerm") {
            return ProtocolType::Iterm2;
        }
    }

    // No hardcoded stuff worked, try querying the terminal now.
    #[cfg(all(feature = "rustix", unix))]
    if let Ok(t) = check_device_attrs() {
        return t;
    }

    ProtocolType::Halfblocks
}

#[allow(unused)]
fn guess_protocol_magic_env_vars() -> ProtocolType {
    let vars = [
        ("KITTY_WINDOW_ID", ProtocolType::Kitty),
        ("ITERM_SESSION_ID", ProtocolType::Iterm2),
        ("WEZTERM_EXECUTABLE", ProtocolType::Iterm2),
    ];
    match vars.into_iter().find(|v| env_exists(v.0)) {
        Some(v) => return v.1,
        None => {
            eprintln!("no special environment variables detected");
        }
    }

    ProtocolType::Halfblocks
}

#[inline]
pub fn env_exists(name: &str) -> bool {
    env::var_os(name).is_some_and(|s| !s.is_empty())
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
    // Disable canonical mode to read without waiting for Enter, disable echoing.
    termios.local_modes &= !LocalModes::ICANON;
    termios.local_modes &= !LocalModes::ECHO;
    rustix::termios::tcsetattr(stdin, OptionalActions::Drain, &termios)?;

    // Enable nonblocking mode for reads, so that this works even if the terminal emulator doesn't
    // reply at all or replies with unexpected data.
    let fd_flags_original = rustix::fs::fcntl_getfl(stdin)?;
    let mut fd_flags = fd_flags_original;
    fd_flags.insert(rustix::fs::OFlags::NONBLOCK);
    rustix::fs::fcntl_setfl(stdin, fd_flags)?;

    rustix::io::write(
        rustix::stdio::stdout(),
        // Queries first for kitty support with `_Gi=...<ESC>\` and then for "graphics attributes"
        // (sixel) with `<ESC>[c`.
        // See https://sw.kovidgoyal.net/kitty/graphics-protocol/#querying-support-and-available-transmission-mediums
        b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[c",
    )?;

    let buf = read_stdin(1000, move || {
        let mut charbuf: [u8; 1] = [0];
        match rustix::io::read(stdin, &mut charbuf) {
            Ok(_) => Ok(charbuf[0]),
            Err(err) => Err(err.into()),
        }
    })?;

    // Reset to previous mode and status, and termios attributes.
    rustix::fs::fcntl_setfl(stdin, fd_flags_original)?;
    rustix::termios::tcsetattr(stdin, OptionalActions::Now, &termios_original)?;

    if buf.contains("_Gi=31;OK") {
        return Ok(ProtocolType::Kitty);
    }
    if buf.contains(";4;") || buf.contains("?4;") || buf.contains(";4c") || buf.contains("?4c") {
        return Ok(ProtocolType::Sixel);
    }
    Err("graphics support not detected".into())
}

pub fn read_stdin(
    timeout_ms: u128,
    mut read: impl FnMut() -> io::Result<u8>,
) -> io::Result<String> {
    let start = Instant::now();
    let mut buf = String::with_capacity(200);
    loop {
        let result = read();
        if Instant::now().duration_since(start).as_millis() > timeout_ms {
            // Always timeout, otherwise the terminal could potentially keep sending data forever.
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                match result {
                    Err(err) => err.to_string(),
                    Ok(_) => "timed out while reading data".to_string(),
                },
            ));
        }
        match result {
            Ok(ch) => {
                buf.push(char::from(ch));
            }
            Err(err) => {
                if err.kind() == io::ErrorKind::WouldBlock {
                    if buf.is_empty() {
                        // No data yet, keep polling.
                        continue;
                    }
                    // We've read some data.
                    return Ok(buf);
                }
                // Some other kind of read error.
                return Err(err);
            }
        }
    }
}

#[cfg(all(test, feature = "rustix"))]
mod tests {
    use std::{
        assert_eq,
        io::{self},
    };

    use crate::picker::{font_size, read_stdin, Picker, ProtocolType};
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

    #[derive(Clone)]
    struct TestStdin<'a> {
        wouldblock_count: u32,
        data: &'a str,
    }

    fn test_stdin(wouldblock_count: u32, data: &str) -> TestStdin {
        TestStdin {
            wouldblock_count,
            data,
        }
    }

    fn read(stdin: &mut TestStdin) -> io::Result<u8> {
        if stdin.wouldblock_count > 0 {
            stdin.wouldblock_count -= 1;
            return Err(io::Error::new(io::ErrorKind::WouldBlock, "waiting"));
        }
        match stdin.data {
            "" => Err(io::Error::new(io::ErrorKind::WouldBlock, "done")),
            data => {
                stdin.data = &stdin.data[1..];
                Ok(data.chars().next().unwrap() as u8)
            }
        }
    }

    #[test]
    fn test_read_stdin() {
        let mut stdin = test_stdin(10, "abcabc");
        assert_eq!("abcabc", read_stdin(20, || read(&mut stdin)).unwrap());
    }

    #[test]
    fn test_read_stdin_timeout() {
        let mut stdin = test_stdin(u32::MAX, "abc");
        let err = read_stdin(1, || read(&mut stdin)).unwrap_err();
        assert_eq!(io::ErrorKind::TimedOut, err.kind());
    }

    #[test]
    fn test_read_stdin_timeout_empty() {
        let mut stdin = test_stdin(0, "");
        let err = read_stdin(1, || read(&mut stdin)).unwrap_err();
        assert_eq!(io::ErrorKind::TimedOut, err.kind());
    }

    #[test]
    fn test_read_stdin_timeout_neverending_data() {
        let data: String = "a".repeat(10000000);
        let mut stdin = test_stdin(1, &data);
        let err = read_stdin(1, || read(&mut stdin)).unwrap_err();
        assert_eq!(io::ErrorKind::TimedOut, err.kind());
    }
}
