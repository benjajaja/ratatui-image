use std::cmp::min;

use backend::{round_size_to_cells, DynamicBackend, StaticBackend};
use image::DynamicImage;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{StatefulWidget, Widget},
};

pub mod backend;

#[derive(Clone)]
pub struct ImageSource {
    pub image: DynamicImage,
    pub font_size: (u16, u16),
    pub rect: Rect,
}

impl ImageSource {
    pub fn new(image: DynamicImage, font_size: (u16, u16)) -> ImageSource {
        let rect = round_size_to_cells(image.width(), image.height(), font_size);
        ImageSource {
            image,
            font_size,
            rect,
        }
    }
}

pub struct StaticImage<'a> {
    image: &'a dyn StaticBackend,
}

impl<'a> StaticImage<'a> {
    pub fn new(image: &'a dyn StaticBackend) -> StaticImage<'a> {
        StaticImage { image }
    }
}

impl<'a> Widget for StaticImage<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        self.image.render(area, buf);
    }
}

pub struct ResizableImage<'a> {
    source: &'a ImageSource,
    resize: Resize,
}

impl<'a> ResizableImage<'a> {
    pub fn new(source: &'a ImageSource) -> ResizableImage<'a> {
        ResizableImage {
            source,
            resize: Resize::Fit,
        }
    }
    pub fn resize(mut self, resize: Resize) -> ResizableImage<'a> {
        self.resize = resize;
        self
    }
}

impl<'a> StatefulWidget for ResizableImage<'a> {
    type State = Box<dyn DynamicBackend>;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        state.render(self.source, &self.resize, area, buf)
    }
}

#[derive(Debug)]
pub enum Resize {
    /// Fit to area
    ///
    /// If the width or height is smaller than the area, the image will be resized maintaining
    /// proportions.
    Fit,
    /// Crop to area
    ///
    /// If the width or height is smaller than the area, the image will be cropped.
    /// The behaviour is the same as using `[StaticImage]` widget with the overhead of resizing,
    /// but some terminals might misbehave on overdrawing graphics.
    Crop,
}

impl Resize {
    fn resize(&self, source: &ImageSource, current: Rect, area: Rect) -> Option<Rect> {
        match self {
            Self::Fit => {
                let desired = source.rect;
                if desired.width <= area.width
                    && desired.height <= area.height
                    && desired == current
                {
                    return None;
                }
                let mut resized = desired;
                if desired.width > area.width {
                    resized.width = area.width;
                    resized.height = ((desired.height as f32)
                        * (area.width as f32 / desired.width as f32))
                        .round() as u16;
                } else if desired.height > area.height {
                    resized.height = area.height;
                    resized.width = ((desired.width as f32)
                        * (area.height as f32 / desired.height as f32))
                        .round() as u16;
                }
                Some(resized)
            }
            Self::Crop => {
                let desired = source.rect;
                if desired.width <= area.width
                    && desired.height <= area.height
                    && desired == current
                {
                    return None;
                }

                Some(Rect::new(
                    0,
                    0,
                    min(desired.width, area.width),
                    min(desired.height, area.height),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use image::{ImageBuffer, Rgb};

    use super::*;

    fn s(w: u16, h: u16, font_size: (u16, u16)) -> ImageSource {
        let image: DynamicImage = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(w as _, h as _).into();
        ImageSource::new(image, font_size)
    }

    fn r(w: u16, h: u16) -> Rect {
        Rect::new(0, 0, w, h)
    }

    #[test]
    fn resize_fit() {
        let resize = Resize::Fit;

        let to = resize.resize(&s(100, 100, (10, 10)), r(10, 10), r(10, 10));
        assert_eq!(None, to);

        let to = resize.resize(&s(80, 100, (10, 10)), r(8, 10), r(10, 10));
        assert_eq!(None, to);

        let to = resize.resize(&s(100, 100, (10, 10)), r(10, 10), r(8, 10));
        assert_eq!(Some(r(8, 8)), to);

        let to = resize.resize(&s(100, 100, (10, 10)), r(10, 10), r(10, 8));
        assert_eq!(Some(r(8, 8)), to);
    }

    #[test]
    fn resize_crop() {
        let resize = Resize::Crop;

        let to = resize.resize(&s(100, 100, (10, 10)), r(10, 10), r(10, 10));
        assert_eq!(None, to);

        let to = resize.resize(&s(80, 100, (10, 10)), r(8, 10), r(10, 10));
        assert_eq!(None, to);

        let to = resize.resize(&s(100, 100, (10, 10)), r(10, 10), r(8, 10));
        assert_eq!(Some(r(8, 10)), to);

        let to = resize.resize(&s(100, 100, (10, 10)), r(10, 10), r(10, 8));
        assert_eq!(Some(r(10, 8)), to);
    }
}
