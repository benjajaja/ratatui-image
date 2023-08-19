//! Backends for the widgets
use std::cmp::min;

use dyn_clone::DynClone;
use image::Rgb;
use ratatui::{buffer::Buffer, layout::Rect};

use crate::ImageSource;

use super::Resize;

pub mod halfblocks;
pub mod kitty;
#[cfg(feature = "sixel")]
pub mod sixel;

/// A fixed image backend for the [crate::FixedImage] widget.
pub trait FixedBackend: Send + Sync {
    fn rect(&self) -> Rect;
    fn render(&self, area: Rect, buf: &mut Buffer);
}

/// A resizing image backend for the [crate::ResizeImage] widget.
pub trait ResizeBackend: Send + Sync + DynClone {
    fn rect(&self) -> Rect;
    fn render(
        &mut self,
        source: &ImageSource,
        resize: &Resize,
        background_color: Option<Rgb<u8>>,
        area: Rect,
        buf: &mut Buffer,
    );
    /// This method is optional.
    fn reset(&mut self) {}
}

dyn_clone::clone_trait_object!(ResizeBackend);

fn render_area(rect: Rect, area: Rect, overdraw: bool) -> Option<Rect> {
    if overdraw {
        return Some(Rect::new(
            area.x,
            area.y,
            min(rect.width, area.width),
            min(rect.height, area.height),
        ));
    }

    if rect.width > area.width || rect.height > area.height {
        return None;
    }
    Some(Rect::new(area.x, area.y, rect.width, rect.height))
}
