//! Helper module to build a backend, and swap backends at runtime
use image::{DynamicImage, Rgb};
use ratatui::{backend::Backend, layout::Rect, Terminal};

#[cfg(feature = "sixel")]
use crate::backend::sixel::{resizeable::SixelState, FixedSixel};

use crate::{
    backend::{
        halfblocks::{resizeable::HalfblocksState, FixedHalfblocks},
        FixedBackend, ResizeBackend,
    },
    ImageSource, Resize, Result,
};

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

/// Backend builder
///
/// Does not hold the static or dynamic states, only the ImageSource.
impl Picker {
    /// Pick backends for the widgets
    ///
    /// TODO: does not really guess and always picks `Sixel`.
    ///
    /// Can cycle through backends with `next()`, or use `set(Current)` e.g. from a user
    /// configuration.
    ///
    /// # Example
    /// ```rust
    /// use std::io;
    /// use ratatui_imagine::{
    ///     picker::Picker,
    ///     Resize,
    /// };
    /// use ratatui::{
    ///     backend::{Backend, TestBackend},
    ///     layout::Rect,
    ///     Terminal,
    /// };
    ///
    /// let mut stdout = io::stdout();
    /// let backend = TestBackend::new(80, 35);
    /// let mut terminal = Terminal::new(backend).unwrap();
    /// let dyn_img = image::io::Reader::open("./assets/Ada.png").unwrap().decode().unwrap();
    /// let picker = Picker::guess(dyn_img, &mut terminal, None).unwrap();
    /// // For FixedImage:
    /// let image_static = picker.new_static_fit(Resize::Fit, Rect::new(0, 0, 15, 5)).unwrap();
    /// // For ResizeImage:
    /// let image_fit_state = picker.new_state();
    /// ```
    #[allow(clippy::vec_init_then_push)]
    pub fn guess<B: Backend>(
        image: DynamicImage,
        terminal: &mut Terminal<B>,
        background_color: Option<Rgb<u8>>,
    ) -> Result<Picker> {
        let source = ImageSource::with_terminal(image, terminal, background_color)?;
        let mut available = vec![];
        #[cfg(feature = "sixel")]
        available.push(Current::Sixel);
        available.push(Current::Halfblocks);

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

    /// Returns a new *static* backend for [`crate::FixedImage`] widgets that fits into the given size.
    pub fn new_static_fit(&self, resize: Resize, area: Rect) -> Result<Box<dyn FixedBackend>> {
        match self.current {
            Current::Halfblocks => Ok(Box::new(FixedHalfblocks::from_source(
                &self.source,
                resize,
                area,
            )?)),
            #[cfg(feature = "sixel")]
            Current::Sixel => Ok(Box::new(FixedSixel::from_source(
                &self.source,
                resize,
                area,
            )?)),
        }
    }

    /// Returns a new *state* backend for [`crate::ResizeImage`].
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
