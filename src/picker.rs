//! Helper module to build a protocol, and swap protocols at runtime

use std::{env, io};

use image::{DynamicImage, Rgb};
use ratatui::layout::Rect;
#[cfg(all(feature = "rustix", unix))]
use rustix::{
    stdio::stdout,
    termios::{tcgetwinsize, Winsize},
};
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

#[derive(Clone, Copy, Debug)]
pub struct Picker {
    pub font_size: FontSize,
    pub background_color: Option<Rgb<u8>>,
    pub protocol_type: ProtocolType,
    pub is_tmux: bool,
    kitty_counter: u32,
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
    /// Query terminal stdio for graphics capabilities and font-size with some escape sequences.
    ///
    /// This writes and reads from stdio momentarily. WARNING: this method should be called after
    /// entering alternate screen but before reading terminal events.
    ///
    /// # Example
    /// ```rust
    /// use ratatui_image::picker::Picker;
    /// let mut picker = Picker::from_query_stdio();
    /// ```
    #[cfg(all(feature = "rustix", unix))]
    pub fn from_query_stdio() -> Result<Picker> {
        let (protocol_type, font_size, is_tmux) = query_stdio();

        if let Some(font_size) = font_size {
            Ok(Picker {
                font_size,
                background_color: None,
                protocol_type,
                is_tmux,
                kitty_counter: rand::random(),
            })
        } else {
            Err("could not query font size".into())
        }
    }

    /// Create a picker from a given terminal [FontSize].
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
            is_tmux: false,
            kitty_counter: rand::random(),
        }
    }

    /// Query terminal stdio for graphics capabilities and font-size with some escape sequences.
    ///
    /// This writes and reads from stdio momentarily. WARNING: this method should be called after
    /// entering alternate screen but before reading terminal events.
    ///
    pub fn query_stdio(&mut self) -> ProtocolType {
        let font_size;
        (self.protocol_type, font_size, self.is_tmux) = query_stdio();
        if let Some(font_size) = font_size {
            self.font_size = font_size;
        }
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
                self.is_tmux,
                size,
            )?)),
            ProtocolType::Kitty => {
                self.kitty_counter = self.kitty_counter.checked_add(1).unwrap_or(1);
                Ok(Box::new(Kitty::from_source(
                    &source,
                    resize,
                    self.background_color,
                    size,
                    self.kitty_counter,
                    self.is_tmux,
                )?))
            }
            ProtocolType::Iterm2 => Ok(Box::new(FixedIterm2::from_source(
                &source,
                resize,
                self.background_color,
                self.is_tmux,
                size,
            )?)),
        }
    }

    /// Returns a new *stateful* protocol for [`crate::StatefulImage`] widgets.
    pub fn new_resize_protocol(&mut self, image: DynamicImage) -> Box<dyn StatefulProtocol> {
        let source = ImageSource::new(image, self.font_size);
        match self.protocol_type {
            ProtocolType::Halfblocks => Box::new(StatefulHalfblocks::new(source)),
            ProtocolType::Sixel => Box::new(StatefulSixel::new(source, self.is_tmux)),
            ProtocolType::Kitty => {
                self.kitty_counter = self.kitty_counter.checked_add(1).unwrap_or(1);
                Box::new(StatefulKitty::new(source, self.kitty_counter, self.is_tmux))
            }
            ProtocolType::Iterm2 => Box::new(Iterm2State::new(source, self.is_tmux)),
        }
    }
}

#[cfg(all(feature = "rustix", unix))]
pub fn from_winsize(winsize: Winsize) -> Result<FontSize> {
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

fn query_stdio() -> (ProtocolType, Option<FontSize>, bool) {
    let mut proto = ProtocolType::Halfblocks;
    let mut font_size = None;
    let mut is_tmux = false;

    // Check if we're inside tmux.
    if let Ok(term) = env::var("TERM") {
        if term.starts_with("tmux") {
            is_tmux = true;
        }
    }
    if let Ok(term_program) = env::var("TERM_PROGRAM") {
        if term_program == "tmux" {
            is_tmux = true;
        }
    }

    if is_tmux {
        let _ = std::process::Command::new("tmux")
            .args(["set", "-p", "allow-passthrough", "on"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .and_then(|mut child| child.wait()); // wait(), for check_device_attrs.

        // Only if we're in tmux, take a risky guess because $TERM has been overwritten.
        // The core issue is that iterm2 support cannot be queried, like kitty or sixel.
        if let Some(magic_proto) = guess_protocol_magic_env_var_exist() {
            proto = magic_proto
        }
    }

    #[cfg(all(feature = "rustix", unix))]
    if let Ok((cap_proto, cap_font_size)) = query_device_attrs(is_tmux) {
        if let Some(cap_proto) = cap_proto {
            proto = cap_proto;
        }
        font_size = cap_font_size.or_else(|| {
            // Couldn't get font size by query, use tcgetwinsize.
            let winsize = tcgetwinsize(stdout()).ok()?;
            from_winsize(winsize).ok()
        })
    }

    // Ideally the capabilities should be the best indication. In practice, some protocols
    // are buggy on specific terminals, and we can pick another one that works better.
    if let Ok(term) = env::var("TERM") {
        if term == "mlterm" || term == "yaft-256color" {
            proto = ProtocolType::Sixel;
        }
        if term.contains("kitty") {
            proto = ProtocolType::Kitty;
        }
    }
    if let Ok(term_program) = env::var("TERM_PROGRAM") {
        if term_program == "MacTerm" {
            proto = ProtocolType::Sixel;
        }
        if term_program.contains("iTerm")
            || term_program.contains("WezTerm")
            || term_program.contains("mintty")
            || term_program.contains("vscode")
            || term_program.contains("Tabby")
            || term_program.contains("Hyper")
        {
            proto = ProtocolType::Iterm2;
        }
    }
    if let Ok(lc_term) = env::var("LC_TERMINAL") {
        if lc_term.contains("iTerm") {
            proto = ProtocolType::Iterm2;
        }
    }

    // Fallback.
    (proto, font_size, is_tmux)
}

/// Crude guess based on the *existence* of some magic program specific env vars.
/// Produces false positives, for example xterm started from kitty inherits KITTY_WINDOW_ID.
/// Furthermore, tmux shares env vars from the first session, for example tmux started in xterm
/// after a previous tmux session started in kitty, inherits KITTY_WINDOW_ID.
fn guess_protocol_magic_env_var_exist() -> Option<ProtocolType> {
    let vars = [
        ("KITTY_WINDOW_ID", ProtocolType::Kitty),
        ("ITERM_SESSION_ID", ProtocolType::Iterm2),
        ("WEZTERM_EXECUTABLE", ProtocolType::Iterm2),
    ];
    vars.into_iter().find(|v| env_exists(v.0)).map(|v| v.1)
}

#[inline]
pub fn env_exists(name: &str) -> bool {
    env::var_os(name).is_some_and(|s| !s.is_empty())
}

#[cfg(all(feature = "rustix", unix))]
fn query_device_attrs(is_tmux: bool) -> Result<(Option<ProtocolType>, Option<FontSize>)> {
    use rustix::termios::{LocalModes, OptionalActions};

    let stdin = rustix::stdio::stdin();
    let termios_original = rustix::termios::tcgetattr(stdin)?;
    let mut termios = termios_original.clone();
    // Disable canonical mode to read without waiting for Enter, disable echoing.
    termios.local_modes &= !LocalModes::ICANON;
    termios.local_modes &= !LocalModes::ECHO;
    rustix::termios::tcsetattr(stdin, OptionalActions::Drain, &termios)?;

    let (start, escape, end) = if is_tmux {
        ("\x1bPtmux;", "\x1b\x1b", "\x1b\\")
    } else {
        ("", "\x1b", "")
    };

    // Send several control sequences at once:
    // `_Gi=...`: Kitty graphics support.
    // `[c`: Capabilities including sixels.
    // `[16t`: Cell-size (perhaps we should also do `[14t`).
    // `[1337n`: iTerm2 (some terminals implement the protocol but sadly not this custom CSI)
    // `[5n`: Device Status Report, implemented by all terminals, ensure that there is some
    // response and we don't hang reading forever.
    let query = format!("{start}{escape}_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA{escape}\\{escape}[c{escape}[16t{escape}[1337n{escape}[5n{end}");
    rustix::io::write(rustix::stdio::stdout(), query.as_bytes())?;

    let mut parser = Parser::new();
    let mut capabilities = vec![];
    'out: loop {
        let mut charbuf: [u8; 50] = [0; 50];
        let result = rustix::io::read(stdin, &mut charbuf);
        match result {
            Ok(read) => {
                for i in 0..read {
                    if let Some(cap) = parser.push(char::from(charbuf[i])) {
                        if cap == ParsedResponse::Status {
                            break 'out;
                        } else {
                            capabilities.push(cap);
                        }
                    }
                }
            }
            Err(err) => {
                if err.kind() == io::ErrorKind::WouldBlock {
                    if parser.data.len() == 0 {
                        // No data yet, keep polling.
                        continue;
                    }
                    // We've read some data.
                    break;
                }
                // Some other kind of read error.
                return Err(err.into());
            }
        }
    }

    if capabilities.is_empty() {
        return Err("no reply to graphics support query".into());
    }

    // Reset to previous termios attributes.
    rustix::termios::tcsetattr(stdin, OptionalActions::Now, &termios_original)?;

    let protocol = if capabilities.contains(&ParsedResponse::Kitty(true)) {
        Some(ProtocolType::Kitty)
    } else if capabilities.contains(&ParsedResponse::Sixel(true)) {
        Some(ProtocolType::Sixel)
    } else {
        None
    };

    let mut font_size = None;
    for cap in capabilities {
        if let ParsedResponse::CellSize(Some((w, h))) = cap {
            font_size = Some((w, h));
        }
    }
    Ok((protocol, font_size))
}

#[derive(Debug, PartialEq)]
enum ParsedResponse {
    Unknown,
    Garbage,
    Kitty(bool),
    Sixel(bool),
    CellSize(Option<(u16, u16)>),
    Status,
}

struct Parser {
    data: String,
}

impl Parser {
    pub fn new() -> Self {
        Parser {
            data: String::new(),
        }
    }
    pub fn push(self: &mut Self, next: char) -> Option<ParsedResponse> {
        let cap = Self::parse_sequence(&self.data, next);
        match cap {
            ParsedResponse::Unknown => {
                self.data.push(next);
            }
            ParsedResponse::Garbage => {
                self.data = String::from(next);
            }
            _ => {
                self.data = String::new();
                return Some(cap);
            }
        }
        None
    }

    fn parse_sequence(data: &str, next: char) -> ParsedResponse {
        if next == '\x1b' {
            if data.len() == 0 || (!data.starts_with("\x1b[") && !data.starts_with("\x1b_Gi=31;")) {
                return ParsedResponse::Garbage;
            }
        }
        if next == '\\' && data.starts_with("\x1b_Gi=31;") && data.ends_with("\x1b") {
            return ParsedResponse::Kitty(data == "\x1b_Gi=31;OK\x1b");
        }
        if next == 'c' && data.starts_with("\x1b[?") {
            return ParsedResponse::Sixel(
                // This is just easier than actually parsing the string.
                data.contains(";4;")
                    || data.contains("?4;")
                    || data.contains(";4")
                    || data.contains("?4"),
            );
        }
        if next == 't' && data.starts_with("\x1b[6;") {
            let inner: Vec<&str> = data.split(';').collect();
            if let [_, h, w] = inner[..] {
                if let (Ok(h), Ok(w)) = (h.parse::<u16>(), w.parse::<u16>()) {
                    if w > 0 && h > 0 {
                        return ParsedResponse::CellSize(Some((w, h)));
                    }
                }
            }
            return ParsedResponse::CellSize(None);
        }
        if next == 'n' && data == "\x1b[0" {
            return ParsedResponse::Status;
        }
        ParsedResponse::Unknown
    }
}

#[cfg(all(test, feature = "rustix"))]
mod tests {
    use std::assert_eq;

    use crate::picker::{from_winsize, ParsedResponse, Parser, Picker, ProtocolType};
    use rustix::termios::Winsize;

    #[test]
    fn test_font_size() {
        assert!(from_winsize(Winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 10,
            ws_ypixel: 10
        })
        .is_err());
        assert!(from_winsize(Winsize {
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

    #[test]
    fn test_parse_all() {
        let mut parser = Parser::new();
        let mut caps = vec![];
        for ch in "\x1b_Gi=31;OK\x1b\\\x1b[?64;4c\x1b[6;7;14t\x1b[0n".chars() {
            if let Some(cap) = parser.push(ch) {
                caps.push(cap);
            }
        }
        assert_eq!(
            caps,
            vec![
                ParsedResponse::Kitty(true),
                ParsedResponse::Sixel(true),
                ParsedResponse::CellSize(Some((14, 7))),
                ParsedResponse::Status
            ]
        );
    }

    #[test]
    fn test_parse_gibberish() {
        let mut parser = Parser::new();
        let mut caps = vec![];
        for ch in "\x1bhonkey\x1btonkey\x1b[42\x1b\\".chars() {
            if let Some(cap) = parser.push(ch) {
                caps.push(cap);
            }
        }
        assert_eq!(0, caps.len());
    }

    #[test]
    fn test_parse_mixed_gibberish() {
        let mut parser = Parser::new();
        let mut caps = vec![];
        for ch in "\x1bgarbage...\x1b[?64;5c\x1b[0n".chars() {
            if let Some(cap) = parser.push(ch) {
                caps.push(cap);
            }
        }
        assert_eq!(
            caps,
            vec![ParsedResponse::Sixel(false), ParsedResponse::Status]
        );
    }
}
