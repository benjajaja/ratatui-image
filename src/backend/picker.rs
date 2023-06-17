use image::DynamicImage;
use ratatui::{backend::Backend, Terminal};

#[cfg(feature = "sixel")]
use super::sixel::{resizeable::SixelState, FixedSixel};
use super::{
    halfblocks::{resizeable::HalfblocksState, FixedHalfblocks},
    round_pixel_size_to_cells, FixedBackend, ImageSource, ResizeBackend,
};
use crate::Result;

pub struct Picker {
    pub source: ImageSource,
    current: Current,
    available: Vec<Current>,
}

#[derive(PartialEq, Clone, Debug)]
pub enum Current {
    Halfblocks,
    #[cfg(feature = "sixel")]
    Sixel,
}

/// Helper for picking a backend
///
/// Does not hold the static or dynamic states, only the ImageSource.
impl Picker {
    /// Pick a default backend (always picks Halfblocks)
    pub fn guess<B: Backend>(image: DynamicImage, terminal: &mut Terminal<B>) -> Result<Picker> {
        let source = ImageSource::with_terminal(image, terminal)?;
        let mut available = vec![];
        available.push(Current::Halfblocks);
        #[cfg(feature = "sixel")]
        available.push(Current::Sixel);

        // TODO: guess current. Can we guess sixel support reliably?
        Ok(Picker {
            source,
            current: available[0].clone(),
            available,
        })
    }

    /// Set a specific backend
    pub fn set(&mut self, r#type: Current) {
        self.current = r#type;
    }

    /// Returns a new backend for static images that matches the image native size.
    pub fn new_static(&mut self) -> Result<Box<dyn FixedBackend>> {
        let rect = round_pixel_size_to_cells(
            self.source.image.width(),
            self.source.image.height(),
            self.source.font_size,
        );
        self.new_static_fit((rect.width, rect.height))
    }

    /// Returns a new backend for static images that fits into the given size.
    pub fn new_static_fit(&mut self, (width, height): (u16, u16)) -> Result<Box<dyn FixedBackend>> {
        // let rect = Rect::new(0, 0, width, height);
        let source = self.source.resize((width, height));
        match self.current {
            Current::Halfblocks => Ok(Box::new(FixedHalfblocks::from_source(source)?)),
            #[cfg(feature = "sixel")]
            Current::Sixel => Ok(Box::new(FixedSixel::from_source(source)?)),
        }
    }

    /// Returns a new state for dynamic images.
    pub fn new_state(&self) -> Box<dyn ResizeBackend> {
        match self.current {
            Current::Halfblocks => Box::<HalfblocksState>::default(),
            #[cfg(feature = "sixel")]
            Current::Sixel => Box::<SixelState>::default(),
        }
    }

    /// Cycles through available backends
    pub fn next(&mut self) {
        if let Some(mut i) = self.available.iter().position(|a| a == &self.current) {
            if i >= self.available.len() - 1 {
                i = 0;
            } else {
                i += 1;
            }
            self.current = self.available[i].clone();
        }
    }

    pub fn current(&self) -> String {
        format!("{:?}", self.current)
    }
}
