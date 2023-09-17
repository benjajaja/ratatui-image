//! Protocol backends for the widgets

use dyn_clone::DynClone;
use image::Rgb;
use ratatui::{buffer::Buffer, layout::Rect};

use super::Resize;

pub mod halfblocks;
pub mod kitty;
#[cfg(feature = "sixel")]
pub mod sixel;

/// A fixed image protocol for the [crate::FixedImage] widget.
pub trait Protocol: Send + Sync {
    fn render(&self, area: Rect, buf: &mut Buffer);
}

/// A resizing image protocol for the [crate::ResizeImage] widget.
pub trait ResizeProtocol: Send + Sync + DynClone {
    fn rect(&self) -> Rect;
    fn render(
        &mut self,
        resize: &Resize,
        background_color: Option<Rgb<u8>>,
        area: Rect,
        buf: &mut Buffer,
    );
    /// This method is optional.
    fn reset(&mut self) {}
}

dyn_clone::clone_trait_object!(ResizeProtocol);
