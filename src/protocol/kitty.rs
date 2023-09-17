/// https://sw.kovidgoyal.net/kitty/graphics-protocol/#unicode-placeholders
use std::format;

use base64::{engine::general_purpose, Engine};
use image::{DynamicImage, Rgb};
use ratatui::{buffer::Buffer, layout::Rect};

use crate::{ImageSource, Resize, Result};

use super::{Protocol, ResizeProtocol};

// Fixed Kitty protocol (transmits image data on every render!)
#[derive(Clone, Default)]
pub struct FixedKitty {
    transmit_data: String,
    unique_id: u8,
    rect: Rect,
}

impl FixedKitty {
    /// Create a FixedHalfblocks from an image.
    ///
    /// The "resolution" is determined by the font size of the terminal. Smaller fonts will result
    /// in more half-blocks for the same image size. To get a size independent of the font size,
    /// the image could be resized in relation to the font size beforehand.
    pub fn from_source(
        source: &ImageSource,
        resize: Resize,
        background_color: Option<Rgb<u8>>,
        area: Rect,
        id: u8,
    ) -> Result<Self> {
        let (image, desired) = resize
            .resize(source, Rect::default(), area, background_color, false)
            .unwrap_or_else(|| (source.image.clone(), source.desired));

        let transmit_data = transmit_virtual(&image, id);
        Ok(Self {
            transmit_data,
            unique_id: id,
            rect: desired,
        })
    }
}

impl Protocol for FixedKitty {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let mut seq = Some(self.transmit_data.clone());
        render(area, self.rect, buf, self.unique_id, &mut seq);
    }
}

#[derive(Clone)]
pub struct KittyState {
    source: ImageSource,
    pub unique_id: u8,
    rect: Rect,
    hash: u64,
    proto_state: KittyProtoState,
}

#[derive(Default, Clone, PartialEq)]
enum KittyProtoState {
    #[default]
    Place,
    TransmitAndPlace(String),
}

impl KittyState {
    pub fn new(source: ImageSource, id: u8) -> KittyState {
        KittyState {
            source,
            unique_id: id,
            rect: Rect::default(),
            hash: u64::default(),
            proto_state: KittyProtoState::default(),
        }
    }
}

impl ResizeProtocol for KittyState {
    fn rect(&self) -> Rect {
        self.rect
    }
    fn render(
        &mut self,
        resize: &Resize,
        background_color: Option<Rgb<u8>>,
        area: Rect,
        buf: &mut Buffer,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let force = self.source.hash != self.hash;
        if let Some((img, rect)) =
            resize.resize(&self.source, self.rect, area, background_color, force)
        {
            let data = transmit_virtual(&img, self.unique_id);
            self.hash = self.source.hash;
            self.rect = rect;
            self.proto_state = KittyProtoState::TransmitAndPlace(data);
        }

        // Transmit only once
        let mut seq = match &mut self.proto_state {
            KittyProtoState::TransmitAndPlace(seq) => {
                let seq = std::mem::take(seq);
                self.proto_state = KittyProtoState::Place;
                Some(seq)
            }
            KittyProtoState::Place => None,
        };

        render(area, self.rect, buf, self.unique_id, &mut seq);
    }
    fn reset(&mut self) {
        self.rect = Rect::default();
        self.hash = u64::default();
        self.proto_state = KittyProtoState::default();
    }
}

fn render(area: Rect, rect: Rect, buf: &mut Buffer, id: u8, seq: &mut Option<String>) {
    // Draw each line of unicode placeholders but all into the first cell.
    // I couldn't work out actually drawing into each cell of the buffer so
    // that `.set_skip(true)` would be made unnecessary. Maybe some other escape
    // sequence gets sneaked in somehow.
    // It could also be made so that each cell starts and ends its own escape sequence
    // with the image id, but maybe that's worse.
    for y in 0..(area.height.min(rect.height)) {
        let mut symbol = seq.take().unwrap_or_default();

        // Start unicode placeholder sequence
        symbol.push_str(&format!("\x1b[38;5;{id}m"));
        add_placeholder(&mut symbol, 0, y);

        for x in 1..(area.width.min(rect.width)) {
            // Add entire row with positions
            add_placeholder(&mut symbol, x, y);
            // Skip or something may overwrite it
            buf.get_mut(area.left() + x, area.top() + y).set_skip(true);
        }
        symbol.push_str("\x1b[0m"); // Stop messing with styles now.
        buf.get_mut(area.left(), area.top() + y).set_symbol(&symbol);
    }
}

/// Create a kitty escape sequence for transmitting and virtual-placement.
///
/// The image will be transmitted as RGB8 in chunks of 4096 bytes.
/// A "virtual placement" (U=1) is created so that we can place it using unicode placeholders.
/// Removing the placements when the unicode placeholder is no longer there is being handled
/// automatically by kitty.
fn transmit_virtual(img: &DynamicImage, id: u8) -> String {
    let (w, h) = (img.width(), img.height());
    let img_rgb8 = img.to_rgb8();
    let bytes = img_rgb8.as_raw();

    let mut str = String::new();

    let mut payload: String;
    let chunks = bytes.chunks(4000);
    let chunk_count = chunks.len();
    for (i, chunk) in chunks.enumerate() {
        payload = general_purpose::STANDARD.encode(chunk);
        match i {
            0 => {
                // Transmit and virtual-place but keep sending chunks
                let more = if chunk_count > 1 { 1 } else { 0 };
                str.push_str(&format!(
                    "\x1b_Gq=2,i={id},a=T,U=1,f=24,t=d,s={w},v={h},m={more};{payload}\x1b\\"
                ));
            }
            n if n + 1 == chunk_count => {
                // m=0 means over
                str.push_str(&format!("\x1b_Gq=2,i={id},m=0;{payload}\x1b\\"));
            }
            _ => {
                // Keep adding chunks
                str.push_str(&format!("\x1b_Gq=2,i={id},m=1;{payload}\x1b\\"));
            }
        }
    }
    str
}

fn add_placeholder(str: &mut String, x: u16, y: u16) {
    str.push('\u{10EEEE}');
    str.push(diacritic(y));
    str.push(diacritic(x));
}

/// From https://sw.kovidgoyal.net/kitty/_downloads/1792bad15b12979994cd6ecc54c967a6/rowcolumn-diacritics.txt
/// See https://sw.kovidgoyal.net/kitty/graphics-protocol/#unicode-placeholders for further explanation.
static DIACRITICS: [char; 297] = [
    '\u{305}',
    '\u{30D}',
    '\u{30E}',
    '\u{310}',
    '\u{312}',
    '\u{33D}',
    '\u{33E}',
    '\u{33F}',
    '\u{346}',
    '\u{34A}',
    '\u{34B}',
    '\u{34C}',
    '\u{350}',
    '\u{351}',
    '\u{352}',
    '\u{357}',
    '\u{35B}',
    '\u{363}',
    '\u{364}',
    '\u{365}',
    '\u{366}',
    '\u{367}',
    '\u{368}',
    '\u{369}',
    '\u{36A}',
    '\u{36B}',
    '\u{36C}',
    '\u{36D}',
    '\u{36E}',
    '\u{36F}',
    '\u{483}',
    '\u{484}',
    '\u{485}',
    '\u{486}',
    '\u{487}',
    '\u{592}',
    '\u{593}',
    '\u{594}',
    '\u{595}',
    '\u{597}',
    '\u{598}',
    '\u{599}',
    '\u{59C}',
    '\u{59D}',
    '\u{59E}',
    '\u{59F}',
    '\u{5A0}',
    '\u{5A1}',
    '\u{5A8}',
    '\u{5A9}',
    '\u{5AB}',
    '\u{5AC}',
    '\u{5AF}',
    '\u{5C4}',
    '\u{610}',
    '\u{611}',
    '\u{612}',
    '\u{613}',
    '\u{614}',
    '\u{615}',
    '\u{616}',
    '\u{617}',
    '\u{657}',
    '\u{658}',
    '\u{659}',
    '\u{65A}',
    '\u{65B}',
    '\u{65D}',
    '\u{65E}',
    '\u{6D6}',
    '\u{6D7}',
    '\u{6D8}',
    '\u{6D9}',
    '\u{6DA}',
    '\u{6DB}',
    '\u{6DC}',
    '\u{6DF}',
    '\u{6E0}',
    '\u{6E1}',
    '\u{6E2}',
    '\u{6E4}',
    '\u{6E7}',
    '\u{6E8}',
    '\u{6EB}',
    '\u{6EC}',
    '\u{730}',
    '\u{732}',
    '\u{733}',
    '\u{735}',
    '\u{736}',
    '\u{73A}',
    '\u{73D}',
    '\u{73F}',
    '\u{740}',
    '\u{741}',
    '\u{743}',
    '\u{745}',
    '\u{747}',
    '\u{749}',
    '\u{74A}',
    '\u{7EB}',
    '\u{7EC}',
    '\u{7ED}',
    '\u{7EE}',
    '\u{7EF}',
    '\u{7F0}',
    '\u{7F1}',
    '\u{7F3}',
    '\u{816}',
    '\u{817}',
    '\u{818}',
    '\u{819}',
    '\u{81B}',
    '\u{81C}',
    '\u{81D}',
    '\u{81E}',
    '\u{81F}',
    '\u{820}',
    '\u{821}',
    '\u{822}',
    '\u{823}',
    '\u{825}',
    '\u{826}',
    '\u{827}',
    '\u{829}',
    '\u{82A}',
    '\u{82B}',
    '\u{82C}',
    '\u{82D}',
    '\u{951}',
    '\u{953}',
    '\u{954}',
    '\u{F82}',
    '\u{F83}',
    '\u{F86}',
    '\u{F87}',
    '\u{135D}',
    '\u{135E}',
    '\u{135F}',
    '\u{17DD}',
    '\u{193A}',
    '\u{1A17}',
    '\u{1A75}',
    '\u{1A76}',
    '\u{1A77}',
    '\u{1A78}',
    '\u{1A79}',
    '\u{1A7A}',
    '\u{1A7B}',
    '\u{1A7C}',
    '\u{1B6B}',
    '\u{1B6D}',
    '\u{1B6E}',
    '\u{1B6F}',
    '\u{1B70}',
    '\u{1B71}',
    '\u{1B72}',
    '\u{1B73}',
    '\u{1CD0}',
    '\u{1CD1}',
    '\u{1CD2}',
    '\u{1CDA}',
    '\u{1CDB}',
    '\u{1CE0}',
    '\u{1DC0}',
    '\u{1DC1}',
    '\u{1DC3}',
    '\u{1DC4}',
    '\u{1DC5}',
    '\u{1DC6}',
    '\u{1DC7}',
    '\u{1DC8}',
    '\u{1DC9}',
    '\u{1DCB}',
    '\u{1DCC}',
    '\u{1DD1}',
    '\u{1DD2}',
    '\u{1DD3}',
    '\u{1DD4}',
    '\u{1DD5}',
    '\u{1DD6}',
    '\u{1DD7}',
    '\u{1DD8}',
    '\u{1DD9}',
    '\u{1DDA}',
    '\u{1DDB}',
    '\u{1DDC}',
    '\u{1DDD}',
    '\u{1DDE}',
    '\u{1DDF}',
    '\u{1DE0}',
    '\u{1DE1}',
    '\u{1DE2}',
    '\u{1DE3}',
    '\u{1DE4}',
    '\u{1DE5}',
    '\u{1DE6}',
    '\u{1DFE}',
    '\u{20D0}',
    '\u{20D1}',
    '\u{20D4}',
    '\u{20D5}',
    '\u{20D6}',
    '\u{20D7}',
    '\u{20DB}',
    '\u{20DC}',
    '\u{20E1}',
    '\u{20E7}',
    '\u{20E9}',
    '\u{20F0}',
    '\u{2CEF}',
    '\u{2CF0}',
    '\u{2CF1}',
    '\u{2DE0}',
    '\u{2DE1}',
    '\u{2DE2}',
    '\u{2DE3}',
    '\u{2DE4}',
    '\u{2DE5}',
    '\u{2DE6}',
    '\u{2DE7}',
    '\u{2DE8}',
    '\u{2DE9}',
    '\u{2DEA}',
    '\u{2DEB}',
    '\u{2DEC}',
    '\u{2DED}',
    '\u{2DEE}',
    '\u{2DEF}',
    '\u{2DF0}',
    '\u{2DF1}',
    '\u{2DF2}',
    '\u{2DF3}',
    '\u{2DF4}',
    '\u{2DF5}',
    '\u{2DF6}',
    '\u{2DF7}',
    '\u{2DF8}',
    '\u{2DF9}',
    '\u{2DFA}',
    '\u{2DFB}',
    '\u{2DFC}',
    '\u{2DFD}',
    '\u{2DFE}',
    '\u{2DFF}',
    '\u{A66F}',
    '\u{A67C}',
    '\u{A67D}',
    '\u{A6F0}',
    '\u{A6F1}',
    '\u{A8E0}',
    '\u{A8E1}',
    '\u{A8E2}',
    '\u{A8E3}',
    '\u{A8E4}',
    '\u{A8E5}',
    '\u{A8E6}',
    '\u{A8E7}',
    '\u{A8E8}',
    '\u{A8E9}',
    '\u{A8EA}',
    '\u{A8EB}',
    '\u{A8EC}',
    '\u{A8ED}',
    '\u{A8EE}',
    '\u{A8EF}',
    '\u{A8F0}',
    '\u{A8F1}',
    '\u{AAB0}',
    '\u{AAB2}',
    '\u{AAB3}',
    '\u{AAB7}',
    '\u{AAB8}',
    '\u{AABE}',
    '\u{AABF}',
    '\u{AAC1}',
    '\u{FE20}',
    '\u{FE21}',
    '\u{FE22}',
    '\u{FE23}',
    '\u{FE24}',
    '\u{FE25}',
    '\u{FE26}',
    '\u{10A0F}',
    '\u{10A38}',
    '\u{1D185}',
    '\u{1D186}',
    '\u{1D187}',
    '\u{1D188}',
    '\u{1D189}',
    '\u{1D1AA}',
    '\u{1D1AB}',
    '\u{1D1AC}',
    '\u{1D1AD}',
    '\u{1D242}',
    '\u{1D243}',
    '\u{1D244}',
];
fn diacritic(y: u16) -> char {
    if y >= DIACRITICS.len() as u16 {
        DIACRITICS[0]
    } else {
        DIACRITICS[y as usize]
    }
}
