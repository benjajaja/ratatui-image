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
    /// Query terminal for font-size with some escape sequence.
    ///
    /// This writes and reads from stdin momentarily. WARNING: this method should be called after
    /// entering alternate screen but before reading terminal events.
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
            is_tmux: false,
            kitty_counter: rand::random(),
        }
    }

    /// Guess the best protocol for the current terminal by issuing some escape sequences to
    /// stdout.
    ///
    /// WARNING: this method should be called after entering alternate screen but before reading
    /// terminal events.
    pub fn guess_protocol(&mut self) -> ProtocolType {
        let font_size;
        (self.protocol_type, font_size, self.is_tmux) = guess_capabilities();
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

    /// Returns a new *resize* protocol for [`crate::StatefulImage`] widgets.
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

// Guess what protocol should be used, first from some program-specific magical env vars, then with
// the typical $TERM* env vars, and then with termios stdin/out queries.
fn guess_capabilities() -> (ProtocolType, Option<FontSize>, bool) {
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
        font_size = cap_font_size;
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

/// Crude guess based on the *existance* of some magic program specific env vars.
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
/// Check for kitty or sixel terminal support.
/// Sadly, iterm2 has no spec for querying support.
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
///
/// NOTE: "tested" means that it guesses correctly, not necessarily rendering correctly.
fn query_device_attrs(is_tmux: bool) -> Result<(Option<ProtocolType>, Option<FontSize>)> {
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
    let fd_flags_original = rustix::fs::fcntl_getfl(stdin).and_then(|fd_flags_original| {
        let mut fd_flags = fd_flags_original;
        fd_flags.insert(rustix::fs::OFlags::NONBLOCK);
        rustix::fs::fcntl_setfl(stdin, fd_flags).map(|_| fd_flags_original)
    });

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

    let buf = read_stdin(
        1000,
        move || {
            let mut charbuf: [u8; 1] = [0];
            match rustix::io::read(stdin, &mut charbuf) {
                Ok(_) => Ok(charbuf[0]),
                Err(err) => Err(err.into()),
            }
        },
        fd_flags_original.is_ok(),
    )?;
    if buf.is_empty() {
        return Err("no reply to graphics support query".into());
    }

    // Reset to previous mode and status, and termios attributes.
    if let Ok(fd_flags_original) = fd_flags_original {
        rustix::fs::fcntl_setfl(stdin, fd_flags_original)?;
    }
    rustix::termios::tcsetattr(stdin, OptionalActions::Now, &termios_original)?;

    let capabilities = parse_response(buf);

    let protocol = if capabilities.contains(&ParsedResponse::Kitty(true)) {
        Some(ProtocolType::Kitty)
    } else if capabilities.contains(&ParsedResponse::Sixel(true)) {
        Some(ProtocolType::Sixel)
    } else {
        None
    };

    let mut font_size = None;
    for cap in capabilities {
        if let ParsedResponse::FontSize(w, h) = cap {
            font_size = Some((w, h));
        }
    }
    Ok((protocol, font_size))
}

fn parse_response(buf: String) -> Vec<ParsedResponse> {
    enum ParseState {
        Empty,
        EscapeStart,
        Reading,
        KittyReading,
        KittyClosing,
    }
    struct Parser {
        state: ParseState,
        data: String,
    }
    let mut chars = buf.chars();
    let mut state = Parser {
        state: ParseState::Empty,
        data: String::new(),
    };

    let mut capabilities = vec![];
    while let Some(c) = chars.next() {
        match state.state {
            ParseState::Empty => {
                if c == '\x1b' {
                    state.state = ParseState::EscapeStart;
                }
            }
            ParseState::EscapeStart => {
                if c == '[' {
                    state.state = ParseState::Reading;
                } else if c == '_' {
                    state.state = ParseState::KittyReading;
                }
                state.data = String::new();
            }
            ParseState::Reading => {
                if c == '\x1b' {
                    state.state = ParseState::EscapeStart;
                    capabilities.push(parse_sequence(&state.data));
                } else {
                    state.data.push(c);
                }
            }
            ParseState::KittyReading => {
                if c == '\x1b' {
                    state.state = ParseState::KittyClosing;
                } else {
                    state.data.push(c);
                }
            }
            ParseState::KittyClosing => {
                if c == '\\' {
                    state.state = ParseState::EscapeStart;
                    capabilities.push(parse_sequence(&state.data));
                }
            }
        }
    }
    if let ParseState::Reading = state.state {
        capabilities.push(parse_sequence(&state.data));
    }
    capabilities
}

#[derive(Debug, PartialEq)]
enum ParsedResponse {
    Unknown(String),
    Kitty(bool),
    Sixel(bool),
    FontSize(u16, u16),
    Status,
}

fn parse_sequence(data: &str) -> ParsedResponse {
    if data.starts_with("Gi=31;") {
        return ParsedResponse::Kitty(data.ends_with("OK"));
    }
    if data.starts_with('?') && data.ends_with('c') {
        return ParsedResponse::Sixel(
            // This is just easier than actually parsing the string.
            data.contains(";4;")
                || data.contains("?4;")
                || data.contains(";4c")
                || data.contains("?4c"),
        );
    }
    if data.starts_with("6;") && data.ends_with('t') {
        let inner: &Vec<&str> = &data[2..data.len() - 1].split(';').collect();
        match inner[..] {
            [h, w] => {
                if let (Ok(h), Ok(w)) = (h.parse::<u16>(), w.parse::<u16>()) {
                    return ParsedResponse::FontSize(w, h);
                }
            }
            _ => {}
        }
    }
    if data == "0n" {
        return ParsedResponse::Status;
    }
    ParsedResponse::Unknown(data.to_owned())
}

pub fn read_stdin(
    timeout_ms: u128,
    mut read: impl FnMut() -> io::Result<u8>,
    is_nonblocking: bool,
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
                if !is_nonblocking && ch == b'c' {
                    // If we couldn't put stdin into nonblocking mode, exit on first 'c', which is
                    // the end of the Send Device Attributes query. However, a 'c' could also
                    // appear in the response to the kitty support query, which could contain any
                    // error message string. But if there is no more data, then read() will block
                    // forever.
                    return Ok(buf);
                }
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
        assert_eq!("abcabc", read_stdin(20, || read(&mut stdin), true).unwrap());
    }

    #[test]
    fn test_read_stdin_timeout() {
        let mut stdin = test_stdin(u32::MAX, "abc");
        let err = read_stdin(1, || read(&mut stdin), true).unwrap_err();
        assert_eq!(io::ErrorKind::TimedOut, err.kind());
    }

    #[test]
    fn test_read_stdin_timeout_empty() {
        let mut stdin = test_stdin(0, "");
        let err = read_stdin(1, || read(&mut stdin), true).unwrap_err();
        assert_eq!(io::ErrorKind::TimedOut, err.kind());
    }

    #[test]
    fn test_read_stdin_timeout_neverending_data() {
        let data: String = "a".repeat(10000000);
        let mut stdin = test_stdin(1, &data);
        let err = read_stdin(1, || read(&mut stdin), true).unwrap_err();
        assert_eq!(io::ErrorKind::TimedOut, err.kind());
    }

    #[test]
    fn test_read_stdin_blocking() {
        let mut stdin = test_stdin(10, "abcabc");
        assert_eq!("abc", read_stdin(20, || read(&mut stdin), false).unwrap());
    }
}
