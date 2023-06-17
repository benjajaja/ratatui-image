use std::io;

use dyn_clone::DynClone;
use image::{imageops::FilterType, DynamicImage};
use ratatui::{backend::Backend, buffer::Buffer, layout::Rect, Terminal};

use crate::ImageSource;

use self::{
    halfblocks::StaticHalfBlocks,
    sixel::{resizeable::SixelState, StaticSixel},
};

use super::Resize;

pub mod halfblocks;
pub mod sixel;

// A static image backend that just holds image data and character size
pub trait StaticBackend {
    // fn data(&self) -> &str;
    // fn size(&self) -> Rect;
    fn render(&self, area: Rect, buf: &mut Buffer);
}

// A resizeable imagen backend
// Resizes itself from `[ResizableImageBackend]`'s render
pub trait DynamicBackend: DynClone {
    fn render(&mut self, source: &ImageSource, resize: &Resize, area: Rect, buf: &mut Buffer);
}

dyn_clone::clone_trait_object!(DynamicBackend);

pub fn pick_static<B: Backend>(
    img: DynamicImage,
    terminal: &mut Terminal<B>,
) -> Result<Box<dyn StaticBackend>, io::Error> {
    Ok(Box::new(StaticHalfBlocks::from_image(img, terminal)?))
    // Ok(Box::new(StaticSixel::from_image(img, terminal)?))
}

pub fn pick_resizeable<B: Backend>(
    img: DynamicImage,
    terminal: &mut Terminal<B>,
) -> Result<(ImageSource, Box<dyn DynamicBackend>), io::Error> {
    let state = Box::<SixelState>::default();
    let font_size = terminal.backend_mut().font_size()?;
    let source = ImageSource::new(img, font_size);
    Ok((source, state))
}

pub fn round_size_to_cells(
    img_width: u32,
    img_height: u32,
    (char_width, char_height): (u16, u16),
) -> Rect {
    let width = (img_width as f32 / char_width as f32).round() as u16;
    let height = (img_height as f32 / char_height as f32).round() as u16;
    Rect::new(0, 0, width, height)
}

fn img_resize(
    img: &DynamicImage,
    (char_width, char_height): (u16, u16),
    rect: Rect,
) -> DynamicImage {
    img.resize_exact(
        (rect.width * char_width) as u32,
        (rect.height * char_height) as u32,
        FilterType::Nearest,
    )
}

fn img_crop(img: &DynamicImage, (char_width, char_height): (u16, u16), rect: Rect) -> DynamicImage {
    img.crop_imm(
        0,
        0,
        (rect.width * char_width) as u32,
        (rect.height * char_height) as u32,
    )
}
