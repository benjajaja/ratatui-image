use std::{
    io,
    sync::mpsc::{self, Sender},
    thread,
    time::Duration,
};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use image::Rgb;
use ratatui::{
    backend::CrosstermBackend,
    layout::Rect,
    prelude::{Backend, Buffer},
    terminal::Frame,
    widgets::{Block, Borders, Paragraph, StatefulWidget},
    Terminal,
};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, Resize};

struct App {
    async_state: ThreadProtocol,
}

enum AppEvent {
    KeyEvent(KeyEvent),
    Redraw(Box<dyn StatefulProtocol>),
}

/// A widget that uses a custom ThreadProtocol as state to offload resizing and encoding to a
/// background thread.
pub struct ThreadImage {
    resize: Resize,
}

impl ThreadImage {
    pub fn new() -> ThreadImage {
        ThreadImage {
            resize: Resize::Fit(None),
        }
    }

    pub fn resize(mut self, resize: Resize) -> ThreadImage {
        self.resize = resize;
        self
    }
}

impl StatefulWidget for ThreadImage {
    type State = ThreadProtocol;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.inner = match state.inner.take() {
            // We have the `protocol` and should either resize or render.
            Some(mut protocol) => {
                // If it needs resizing (grow or shrink) then send it away instead of rendering.
                if let Some(rect) = protocol.needs_resize(&self.resize, area) {
                    state.tx.send((protocol, self.resize, rect)).unwrap();
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

/// The state of a ThreadImage.
///
/// Has `inner` [ResizeProtocol] that is sent off to the `tx` mspc channel to do the
/// `resize_encode()` work.
pub struct ThreadProtocol {
    inner: Option<Box<dyn StatefulProtocol>>,
    tx: Sender<(Box<dyn StatefulProtocol>, Resize, Rect)>,
}

impl ThreadProtocol {
    pub fn new(
        tx: Sender<(Box<dyn StatefulProtocol>, Resize, Rect)>,
        inner: Box<dyn StatefulProtocol>,
    ) -> ThreadProtocol {
        ThreadProtocol {
            inner: Some(inner),
            tx,
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen,)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut picker = Picker::from_termios()?;
    picker.guess_protocol();
    // picker.protocol_type = ProtocolType::Halfblocks;
    picker.background_color = Some(Rgb::<u8>([255, 0, 255]));
    let dyn_img = image::io::Reader::open("./assets/Ada.png")?.decode()?;

    // Send a [ResizeProtocol] to resize and encode it in a separate thread.
    let (tx_worker, rec_worker) = mpsc::channel::<(Box<dyn StatefulProtocol>, Resize, Rect)>();

    // Send UI-events and the [ResizeProtocol] result back to main thread.
    let (tx_main, rec_main) = mpsc::channel();

    // Resize and encode in background thread.
    let tx_main_render = tx_main.clone();
    thread::spawn(move || loop {
        if let Ok((mut protocol, resize, area)) = rec_worker.recv() {
            protocol.resize_encode(&resize, None, area);
            tx_main_render.send(AppEvent::Redraw(protocol)).unwrap();
        }
    });

    // Poll events in background thread to demonstrate polling terminal events and redraw events
    // concurrently. It's not required to do it this way - the "redraw event" from the channel
    // could be read after polling the terminal events (as long as it's done with a timout). But
    // then the rendering of the image will always be somewhat delayed.
    let tx_main_events = tx_main.clone();
    thread::spawn(move || -> Result<(), std::io::Error> {
        loop {
            if crossterm::event::poll(Duration::from_millis(1000))? {
                if let Event::Key(key) = event::read()? {
                    tx_main_events.send(AppEvent::KeyEvent(key)).unwrap();
                }
            }
        }
    });

    let mut app = App {
        async_state: ThreadProtocol::new(tx_worker, picker.new_resize_protocol(dyn_img)),
    };

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if let Ok(ev) = rec_main.try_recv() {
            match ev {
                AppEvent::KeyEvent(key) => {
                    if key.kind == KeyEventKind::Press {
                        if key.code == KeyCode::Esc {
                            break;
                        }
                    }
                }
                AppEvent::Redraw(protocol) => {
                    app.async_state.inner = Some(protocol);
                }
            }
        }
    }

    // restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
    terminal.show_cursor()?;

    Ok(())
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let area = f.size();
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Screenshot test");

    f.render_widget(
        Paragraph::new("PartiallyHiddenScreenshotParagraphBackground\n".repeat(10)),
        block.inner(area),
    );

    let image = ThreadImage::new().resize(Resize::Fit(None));
    f.render_stateful_widget(image, block.inner(area), &mut app.async_state);
    f.render_widget(block, area);
}
