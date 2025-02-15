//! Widget that separates resize+encode from rendering.
//! This allows for rendering to be non-blocking, offloading resize+encode into another thread.
//! See examples/async.rs for how to setup the threads and channels.
//! At least one worker thread for resize+encode is required, the example shows how to combine
//! the needs-resize-polling with other terminal events into one event loop.

use std::sync::mpsc::Sender;

use ratatui::{
    prelude::{Buffer, Rect},
    widgets::StatefulWidget,
};

use crate::{protocol::StatefulProtocol, Resize};

/// A widget that uses a custom ThreadProtocol as state to offload resizing and encoding to a
/// background thread.
pub struct ThreadImage {
    resize: Resize,
}

impl ThreadImage {
    pub fn resize(mut self, resize: Resize) -> ThreadImage {
        self.resize = resize;
        self
    }
}

impl Default for ThreadImage {
    fn default() -> Self {
        ThreadImage {
            resize: Resize::Fit(None),
        }
    }
}

impl StatefulWidget for ThreadImage {
    type State = ThreadProtocol;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.inner = match state.inner.take() {
            // We have the `protocol` and should either resize or render.
            Some(mut protocol) => {
                // If it needs resizing (grow or shrink) then send it away instead of rendering.
                // Send the requested area instead of the calculated area
                // to ensure consistent calculations between the render thread and the UI thread.
                if let Some(area) = protocol.needs_resize(&self.resize, area) {
                    state.id.1 += 1;
                    state
                        .tx
                        .send((protocol, self.resize, area, state.id))
                        .unwrap();
                    None
                } else {
                    protocol.render(area, buf);
                    Some(protocol)
                }
            }
            // We are waiting to get back the protocol.
            None => None,
        };
    }
}

/// An ID to track if an incoming resized/encoded image matches last render requirements.
///
/// The first element is the generation, which is incremented by [ThreadProtocol::set_new_protocol]
/// if the image is replaced. The second element is the render counter.
pub type RenderId = (usize, usize);

/// The state of a ThreadImage.
///
/// Has `inner` [ResizeProtocol] that is sent off to the `tx` mspc channel to do the
/// `resize_encode()` work.
/// The `id` is used to discard stale updates. For example, while a large image is being resized by the thread,
/// in the meantime the source image could be replaced with a small image that takes less time to
/// resize.
pub struct ThreadProtocol {
    inner: Option<StatefulProtocol>,
    tx: Sender<(StatefulProtocol, Resize, Rect, RenderId)>,
    id: RenderId,
}

impl ThreadProtocol {
    /// Create a new ThreadProtocol from a [std::sync::mpsc::Sender] and an inner
    /// [StatefulProtocol].
    pub fn new(
        tx: Sender<(StatefulProtocol, Resize, Rect, RenderId)>,
        inner: StatefulProtocol,
    ) -> ThreadProtocol {
        ThreadProtocol {
            inner: Some(inner),
            tx,
            id: (0, 0),
        }
    }
    /// Update the protocol (resized/encoded image).
    ///
    /// Typically, this will be called by the thread that processed the
    /// [StatefulProtocol::resize_encode], with the [RenderId] to discard stale resizes.
    pub fn update_protocol(&mut self, proto: StatefulProtocol, id: RenderId) {
        if id == self.id {
            self.inner = Some(proto);
        }
    }
    /// Set a new protocol (image).
    ///
    /// Typically, this would be called if the source image is being changed, to avoid that calls
    /// to [ThreadProtocol::update_protocol] originating from the previous image overwriting the
    /// protocol.
    pub fn set_new_protocol(&mut self, proto: StatefulProtocol) {
        self.id = (self.id.0 + 1, 0);
        self.inner = Some(proto);
    }
}
