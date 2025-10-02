use std::{
    fs,
    sync::mpsc::{self},
    thread,
    time::{self, Duration},
};

use ratatui::{
    Frame,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    layout::{Position, Rect},
    style::{Color, Stylize},
    text::Line,
    widgets::{Block, Borders, Paragraph},
};
use ratatui_image::{
    Resize, StatefulImage,
    errors::Errors,
    picker::Picker,
    thread::{ResizeRequest, ResizeResponse, ThreadProtocol},
};

struct App {
    async_state: ThreadProtocol,
    logo_pos: Position,
    source_code_lines: Vec<String>,
}

enum AppEvent {
    KeyEvent(KeyEvent),
    Redraw(Result<ResizeResponse, Errors>),
    Tick,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut terminal = ratatui::init();

    let picker = Picker::from_query_stdio()?;
    let dyn_img = image::ImageReader::open("./assets/NixOS.png")?.decode()?;

    // Send a [ResizeProtocol] to resize and encode it in a separate thread.
    let (tx_worker, rec_worker) = mpsc::channel::<ResizeRequest>();

    // Send UI-events and the [ResizeProtocol] result back to main thread.
    let (tx_main, rec_main) = mpsc::channel();

    // Resize and encode in background thread.
    let tx_main_render = tx_main.clone();
    thread::spawn(move || {
        loop {
            if let Ok(request) = rec_worker.recv() {
                tx_main_render
                    .send(AppEvent::Redraw(request.resize_encode()))
                    .unwrap();
            }
        }
    });

    // Poll events in background thread to demonstrate polling terminal events and redraw events
    // concurrently. It's not required to do it this way - the "redraw event" from the channel
    // could be read after polling the terminal events (as long as it's done with a timout). But
    // then the rendering of the image will always be somewhat delayed.
    let tx_main_events = tx_main.clone();
    thread::spawn(move || -> Result<(), std::io::Error> {
        loop {
            if ratatui::crossterm::event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    tx_main_events.send(AppEvent::KeyEvent(key)).unwrap();
                }
            } else {
                tx_main_events.send(AppEvent::Tick).unwrap();
            }
        }
    });

    let mut app = App {
        async_state: ThreadProtocol::new(tx_worker, Some(picker.new_resize_protocol(dyn_img))),
        logo_pos: Position { x: 1, y: 1 },
        source_code_lines: Vec::new(),
    };

    let source_code = fs::read_to_string("./examples/thread.rs")?;
    app.source_code_lines = source_code.split("\n").map(|s| s.to_string()).collect();

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if let Ok(ev) = rec_main.try_recv() {
            match ev {
                AppEvent::KeyEvent(key) => {
                    if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                        break;
                    }
                }
                AppEvent::Redraw(completed) => {
                    let _ = app.async_state.update_resized_protocol(completed?);
                }
                AppEvent::Tick => {
                    if app.source_code_lines.len() > 1 {
                        app.source_code_lines.remove(0);
                    } else {
                        app.source_code_lines =
                            source_code.split("\n").map(|s| s.to_string()).collect();
                    }
                }
            }
        }
    }

    ratatui::restore();

    Ok(())
}

fn ui(f: &mut Frame<'_>, app: &mut App) {
    let area = f.area();
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Thread test")
        .bg(Color::Blue);
    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let mut i = 0;
    for y in inner_area.y..inner_area.height {
        if i >= app.source_code_lines.len() {
            break;
        }
        let p = Paragraph::new(app.source_code_lines[i].clone());
        f.render_widget(p, Rect::new(inner_area.x, y, inner_area.width, 1));
        i += 1;
    }

    let image_block = Block::default()
        .borders(Borders::ALL)
        .title("Nix")
        .bg(Color::Reset);
    let size = app
        .async_state
        .size_for(Resize::default(), inner_area)
        .unwrap_or_default();
    let mut area = size;
    area.x = app.logo_pos.x;
    area.y = app.logo_pos.y;

    let inner_area = image_block.inner(area);
    f.render_widget(image_block, area);

    f.render_stateful_widget(StatefulImage::new(), inner_area, &mut app.async_state);
}
