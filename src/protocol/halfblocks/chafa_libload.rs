//! Chafa-based halfblocks implementation using runtime library loading (libloading).
//!
//! Falls back to primitive halfblocks if libchafa is not available at runtime.

use std::ffi::c_void;
use std::sync::OnceLock;

use image::DynamicImage;
use libloading::Library;
use ratatui::{layout::Rect, style::Color};

use super::HalfBlock;

// Opaque pointer types
type ChafaSymbolMap = *mut c_void;
type ChafaCanvasConfig = *mut c_void;
type ChafaCanvas = *mut c_void;

// Constants from chafa.h
// CHAFA_SYMBOL_TAG_ALL = ~(CHAFA_SYMBOL_TAG_EXTRA | CHAFA_SYMBOL_TAG_BAD)
//                      = ~((1 << 30) | (1 << 19) | (1 << 20))
//                      = ~0x40180000 = 0xBFE7FFFF
const CHAFA_SYMBOL_TAG_ALL: u32 = 0xBFE7FFFF;
// CHAFA_PIXEL_RGB8 is the 9th enum value (0-indexed: 8)
const CHAFA_PIXEL_RGB8: u32 = 8;

// Function pointer types
type ChafaSymbolMapNew = unsafe extern "C" fn() -> ChafaSymbolMap;
type ChafaSymbolMapAddByTags = unsafe extern "C" fn(ChafaSymbolMap, u32);
type ChafaSymbolMapUnref = unsafe extern "C" fn(ChafaSymbolMap);
type ChafaCanvasConfigNew = unsafe extern "C" fn() -> ChafaCanvasConfig;
type ChafaCanvasConfigSetSymbolMap = unsafe extern "C" fn(ChafaCanvasConfig, ChafaSymbolMap);
type ChafaCanvasConfigSetGeometry = unsafe extern "C" fn(ChafaCanvasConfig, i32, i32);
type ChafaCanvasConfigUnref = unsafe extern "C" fn(ChafaCanvasConfig);
type ChafaCanvasNew = unsafe extern "C" fn(ChafaCanvasConfig) -> ChafaCanvas;
type ChafaCanvasDrawAllPixels = unsafe extern "C" fn(ChafaCanvas, u32, *const u8, i32, i32, i32);
type ChafaCanvasGetCharAt = unsafe extern "C" fn(ChafaCanvas, i32, i32) -> u32;
type ChafaCanvasGetColorsAt = unsafe extern "C" fn(ChafaCanvas, i32, i32, *mut i32, *mut i32);
type ChafaCanvasUnref = unsafe extern "C" fn(ChafaCanvas);

/// Holds the loaded chafa library and cached symbol map.
struct ChafaLib {
    _lib: Library,
    symbol_map: ChafaSymbolMap,
    // Function pointers
    symbol_map_unref: ChafaSymbolMapUnref,
    canvas_config_new: ChafaCanvasConfigNew,
    canvas_config_set_symbol_map: ChafaCanvasConfigSetSymbolMap,
    canvas_config_set_geometry: ChafaCanvasConfigSetGeometry,
    canvas_config_unref: ChafaCanvasConfigUnref,
    canvas_new: ChafaCanvasNew,
    canvas_draw_all_pixels: ChafaCanvasDrawAllPixels,
    canvas_get_char_at: ChafaCanvasGetCharAt,
    canvas_get_colors_at: ChafaCanvasGetColorsAt,
    canvas_unref: ChafaCanvasUnref,
}

// SAFETY: The chafa library functions are thread-safe for independent canvases.
// The symbol_map is created once and only read afterwards.
unsafe impl Send for ChafaLib {}
unsafe impl Sync for ChafaLib {}

impl Drop for ChafaLib {
    fn drop(&mut self) {
        unsafe {
            (self.symbol_map_unref)(self.symbol_map);
        }
    }
}

static CHAFA: OnceLock<Option<ChafaLib>> = OnceLock::new();

fn load_chafa() -> Option<ChafaLib> {
    unsafe {
        // Try different library names for different platforms
        let lib = Library::new("libchafa.so.0")
            .or_else(|_| Library::new("libchafa.so"))
            .or_else(|_| Library::new("libchafa.dylib"))
            .or_else(|_| Library::new("chafa.dll"))
            .ok()?;

        // Load all function symbols and immediately dereference them
        // so we can move the library into the struct
        let symbol_map_new: ChafaSymbolMapNew = *lib.get(b"chafa_symbol_map_new").ok()?;
        let symbol_map_add_by_tags: ChafaSymbolMapAddByTags =
            *lib.get(b"chafa_symbol_map_add_by_tags").ok()?;
        let symbol_map_unref: ChafaSymbolMapUnref = *lib.get(b"chafa_symbol_map_unref").ok()?;
        let canvas_config_new: ChafaCanvasConfigNew = *lib.get(b"chafa_canvas_config_new").ok()?;
        let canvas_config_set_symbol_map: ChafaCanvasConfigSetSymbolMap =
            *lib.get(b"chafa_canvas_config_set_symbol_map").ok()?;
        let canvas_config_set_geometry: ChafaCanvasConfigSetGeometry =
            *lib.get(b"chafa_canvas_config_set_geometry").ok()?;
        let canvas_config_unref: ChafaCanvasConfigUnref =
            *lib.get(b"chafa_canvas_config_unref").ok()?;
        let canvas_new: ChafaCanvasNew = *lib.get(b"chafa_canvas_new").ok()?;
        let canvas_draw_all_pixels: ChafaCanvasDrawAllPixels =
            *lib.get(b"chafa_canvas_draw_all_pixels").ok()?;
        let canvas_get_char_at: ChafaCanvasGetCharAt =
            *lib.get(b"chafa_canvas_get_char_at").ok()?;
        let canvas_get_colors_at: ChafaCanvasGetColorsAt =
            *lib.get(b"chafa_canvas_get_colors_at").ok()?;
        let canvas_unref: ChafaCanvasUnref = *lib.get(b"chafa_canvas_unref").ok()?;

        // Create and configure the symbol map (cached for reuse)
        let symbol_map = symbol_map_new();
        if symbol_map.is_null() {
            return None;
        }
        symbol_map_add_by_tags(symbol_map, CHAFA_SYMBOL_TAG_ALL);

        Some(ChafaLib {
            _lib: lib,
            symbol_map,
            symbol_map_unref,
            canvas_config_new,
            canvas_config_set_symbol_map,
            canvas_config_set_geometry,
            canvas_config_unref,
            canvas_new,
            canvas_draw_all_pixels,
            canvas_get_char_at,
            canvas_get_colors_at,
            canvas_unref,
        })
    }
}

#[cfg(test)]
/// Returns true if chafa is available at runtime.
pub fn is_available() -> bool {
    CHAFA.get_or_init(load_chafa).is_some()
}

/// Encode using chafa if available, otherwise return None.
pub fn encode(img: &DynamicImage, area: Rect) -> Option<Vec<HalfBlock>> {
    let chafa = CHAFA.get_or_init(load_chafa).as_ref()?;

    let width = area.width;
    let height = area.height;

    unsafe {
        let config = (chafa.canvas_config_new)();
        (chafa.canvas_config_set_symbol_map)(config, chafa.symbol_map);

        (chafa.canvas_config_set_geometry)(config, width as i32, height as i32);
        let canvas = (chafa.canvas_new)(config);

        let rgb = img.to_rgb8();
        let (w, h) = rgb.dimensions();

        (chafa.canvas_draw_all_pixels)(
            canvas,
            CHAFA_PIXEL_RGB8,
            rgb.as_ptr(),
            w as i32,
            h as i32,
            (w * 3) as i32,
        );

        let mut blocks = Vec::with_capacity((width * height) as usize);

        for y in 0..height {
            for x in 0..width {
                let c = (chafa.canvas_get_char_at)(canvas, x as i32, y as i32);
                let symbol = char::from_u32(c).unwrap_or(' ');

                let mut fg_color: i32 = 0;
                let mut bg_color: i32 = 0;
                (chafa.canvas_get_colors_at)(
                    canvas,
                    x as i32,
                    y as i32,
                    &mut fg_color,
                    &mut bg_color,
                );

                let fg = Color::Rgb(
                    ((fg_color >> 16) & 0xff) as u8,
                    ((fg_color >> 8) & 0xff) as u8,
                    (fg_color & 0xff) as u8,
                );
                let bg = Color::Rgb(
                    ((bg_color >> 16) & 0xff) as u8,
                    ((bg_color >> 8) & 0xff) as u8,
                    (bg_color & 0xff) as u8,
                );

                blocks.push(HalfBlock {
                    upper: fg,
                    lower: bg,
                    char: symbol,
                });
            }
        }

        (chafa.canvas_unref)(canvas);
        (chafa.canvas_config_unref)(config);

        Some(blocks)
    }
}
