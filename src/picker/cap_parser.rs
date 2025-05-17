use std::fmt::Write;

pub struct Parser {
    data: String,
    sequence: Response,
}

#[derive(Debug, PartialEq)]
pub enum Response {
    Unknown,
    CSIResponse,
    Kitty,
    DeviceAttributes,
    CellSize,
    CursorPositionReport,
    Status,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Capability {
    Kitty,
    Sixel,
    RectangularOps,
    CellSize(Option<(u16, u16)>),
    CursorPositionReport(u16, u16),
    TextSizingProtocol,
    Status, // Might as well call this "End" internally.
}

#[derive(Debug, PartialEq, Default)]
pub struct DeviceAttributeResponse {
    pub sixel: bool,
    pub rectangular_ops: bool,
}

/// Extra query options
pub struct QueryStdioOptions {
    /// Query for [Text Sizing Protocol]. The result can be checked by searching for
    /// [Capability::TextSizingProtocol] in [crate::picker::Picker::capabilities].
    ///
    /// [Text Sizing Protocol] <https://sw.kovidgoyal.net/kitty/text-sizing-protocol//>
    pub text_sizing_protocol: bool,
}

impl Default for Parser {
    fn default() -> Self {
        Parser {
            data: String::new(),
            sequence: Response::Unknown,
        }
    }
}

impl Parser {
    pub fn new() -> Self {
        Parser {
            data: String::new(),
            sequence: Response::Unknown,
        }
    }
    // Tmux requires escapes to be escaped, and some special start/end sequences.
    pub fn escape_tmux(is_tmux: bool) -> (&'static str, &'static str, &'static str) {
        match is_tmux {
            false => ("", "\x1b", ""),
            true => ("\x1bPtmux;", "\x1b\x1b", "\x1b\\"),
        }
    }
    pub fn query(is_tmux: bool, options: QueryStdioOptions) -> String {
        let (start, escape, end) = Parser::escape_tmux(is_tmux);

        let mut buf = String::with_capacity(100);
        buf.push_str(start);

        // Kitty graphics
        write!(buf, "{escape}_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA{escape}\\").unwrap();

        // Device Attributes Report 1 (sixel support)
        write!(buf, "{escape}[c").unwrap();

        // Font size in pixels
        write!(buf, "{escape}[16t").unwrap();

        // iTerm2 proprietary, unknown response, untested so far.
        //write!(buf, "{escape}[1337n").unwrap();

        if options.text_sizing_protocol {
            const BEL: &str = "\u{7}";
            // CPR
            // https://sw.kovidgoyal.net/kitty/text-sizing-protocol/#detecting-if-the-terminal-supports-this-protocol
            // We need to write CPR, a resized space, and CPR again, to see if it moved the cursor. Do
            // it again for the scaling part of the protocol.
            // If unsupported, all the CPRs will be the same.
            write!(
                buf,
                "{escape}[6n{escape}]66;w=2; {BEL}{escape}[6n{escape}]66;s=2; {BEL}{escape}[6n"
            )
            .unwrap();
        }

        // End with Device Status Report, implemented by all terminals, ensure that there is some
        // response and we don't hang reading forever.
        write!(buf, "{escape}[5n").unwrap();

        write!(buf, "{end}").unwrap();
        buf
    }
    pub fn push(&mut self, next: char) -> Vec<Capability> {
        match self.sequence {
            Response::Unknown => {
                match (&self.data[..], next) {
                    (_, '\x1b') => {
                        // If the current sequence hasn't been identified yet, start a new one on Esc.
                        return self.restart();
                    }
                    ("_Gi=31", ';') => {
                        self.sequence = Response::Kitty;
                    }

                    ("[", _) => {
                        self.sequence = Response::CSIResponse;
                    }
                    _ => {}
                };
                self.data.push(next);
            }
            Response::CSIResponse => {
                if self.data == "[0" && next == 'n' {
                    self.restart();
                    return vec![Capability::Status];
                }
                match next {
                    'c' if self.data.starts_with("[?") => {
                        let mut caps = vec![];
                        let inner: Vec<&str> = (self.data[2..]).split(';').collect();
                        for cap in inner {
                            match cap {
                                "4" => caps.push(Capability::Sixel),
                                "28" => caps.push(Capability::RectangularOps),
                                _ => {}
                            }
                        }
                        self.restart();
                        return caps;
                    }
                    't' => {
                        let mut cell_size = None;
                        println!("t split: {}", self.data);
                        let inner: Vec<&str> = self.data.split(';').collect();
                        if let [_, h, w] = inner[..] {
                            if let (Ok(h), Ok(w)) = (h.parse::<u16>(), w.parse::<u16>()) {
                                if w > 0 && h > 0 {
                                    cell_size = Some((w, h));
                                }
                            }
                        }
                        self.restart();
                        return vec![Capability::CellSize(cell_size)];
                    }
                    'R' => {
                        let mut cursor_pos = None;
                        let inner: Vec<&str> = self.data[1..].split(';').collect();
                        if let [x, w] = inner[..] {
                            if let (Ok(x), Ok(y)) = (x.parse::<u16>(), w.parse::<u16>()) {
                                cursor_pos = Some((y, x));
                            }
                        }
                        if let Some((x, y)) = cursor_pos {
                            self.restart();
                            return vec![Capability::CursorPositionReport(x, y)];
                        } else {
                            println!("BAD CursorPositionReport: {}", self.data);
                            self.restart();
                            return vec![];
                        }
                    }
                    '\x1b' => {
                        // Give up?
                        return self.restart();
                    }
                    _ => {
                        self.data.push(next);
                    }
                };
            }

            Response::Kitty => match next {
                '\\' => {
                    let caps = match &self.data[..] {
                        "_Gi=31;OK\x1b" => vec![Capability::Kitty],
                        _ => vec![],
                    };
                    self.restart();
                    return caps;
                }
                _ => {
                    self.data.push(next);
                }
            },
            _ => {
                debug_assert!(false, "parse while in terminated state");
                self.restart();
            }
        };
        vec![]
    }
    fn restart(&mut self) -> Vec<Capability> {
        self.data = String::new();
        self.sequence = Response::Unknown;
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use std::assert_eq;

    use super::{Capability, Parser};

    #[test]
    fn test_parse_all() {
        for (name, str, expected) in vec![
            (
                "all",
                "\x1b_Gi=31;OK\x1b\\\x1b[?64;4c\x1b[6;7;14t\x1b[6;6R\x1b[7;7R\x1b[6;6R\x1b[0n",
                vec![
                    Capability::Kitty,
                    Capability::Sixel,
                    Capability::CellSize(Some((14, 7))),
                    Capability::CursorPositionReport(6, 6),
                    Capability::CursorPositionReport(7, 7),
                    Capability::CursorPositionReport(6, 6),
                    Capability::Status,
                ],
            ),
            ("only garbage", "\x1bhonkey\x1btonkey\x1b[42\x1b\\", vec![]),
            (
                "preceding garbage",
                "\x1bgarbage...\x1b[?64;5c\x1b[0n",
                vec![Capability::Status],
            ),
            (
                "inner garbage",
                "\x1b[6;7;14t\x1bgarbage...\x1b[?64;5c\x1b[0n",
                vec![Capability::CellSize(Some((14, 7))), Capability::Status],
            ),
        ] {
            let mut parser = Parser::new();
            let mut caps: Vec<Capability> = vec![];
            for ch in str.chars() {
                let mut more_caps = parser.push(ch);
                caps.append(&mut more_caps)
            }
            assert_eq!(caps, expected, "{name}");
        }
    }
}
