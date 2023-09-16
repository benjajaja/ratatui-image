use std::{
    env,
    io::{self, Stdout},
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use image::Rgb;
use ratatu_image::{picker::Picker, protocol::ResizeProtocol, ImageSource, Resize, ResizeImage};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};

struct App {
    pub filename: String,
    pub picker: Picker,
    pub image_source: ImageSource,
    pub image_state: Box<dyn ResizeProtocol>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let filename = env::args()
        .nth(1)
        .expect("Usage: <program> [path/to/image]");
    let image = image::io::Reader::open(&filename)?.decode()?;

    let mut picker = Picker::from_termios(Some(Rgb::<u8>([255, 0, 255])))?;

    let image_source = ImageSource::new(image.clone(), picker.font_size());
    let image_state = picker.new_state(image);

    let mut app = App {
        filename,
        picker,
        image_source,
        image_state,
    };

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(1000);
    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char(c) => match c {
                            'q' => break,
                            ' ' => {
                                app.picker.cycle_backends();
                                app.image_state =
                                    app.picker.new_state(app.image_source.image.clone());
                            }
                            _ => {}
                        },
                        KeyCode::Esc => break,
                        _ => {}
                    }
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
    terminal.show_cursor()?;

    Ok(())
}

fn ui(f: &mut Frame<CrosstermBackend<Stdout>>, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(6), Constraint::Min(1)].as_ref())
        .split(f.size());

    let block_top = Block::default().borders(Borders::ALL).title("ratatu-image");
    let dyn_img = &app.image_source.image;
    let lines = vec![
        Line::from(format!(
            "Terminal: {:?}, font size: {:?}",
            app.picker.backend_type(),
            app.picker.font_size()
        )),
        Line::from(format!("File: {}", app.filename)),
        Line::from(format!(
            "Image: {:?} {:?}",
            (dyn_img.width(), dyn_img.height()),
            dyn_img.color()
        )),
    ];
    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: true }),
        block_top.inner(chunks[0]),
    );
    f.render_widget(block_top, chunks[0]);

    let block_bottom = Block::default().borders(Borders::ALL).title("image");
    let image = ResizeImage::new(None).resize(Resize::Fit);
    f.render_stateful_widget(image, block_bottom.inner(chunks[1]), &mut app.image_state);
    f.render_widget(block_bottom, chunks[1]);
}
