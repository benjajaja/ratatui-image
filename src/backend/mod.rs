//! Backends for the widgets
use dyn_clone::DynClone;
use ratatui::{buffer::Buffer, layout::Rect};

use crate::ImageSource;

use super::Resize;

pub mod halfblocks;
#[cfg(feature = "sixel")]
pub mod sixel;

// A static image backend that just holds image data and character size
pub trait FixedBackend: Send + Sync {
    fn rect(&self) -> Rect;
    fn render(&self, area: Rect, buf: &mut Buffer);
    fn data(&self) -> String;
}

// A resizeable imagen backend
// Resizes itself from `[ResizableImageBackend]`'s render
pub trait ResizeBackend: Send + Sync + DynClone {
    fn render(&mut self, source: &ImageSource, resize: &Resize, area: Rect, buf: &mut Buffer);
}

dyn_clone::clone_trait_object!(ResizeBackend);
