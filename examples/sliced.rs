use std::{fs, time::Duration};

use image::GenericImageView;
use ratatui::{
    Frame,
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
    layout::{Rect, Size},
    style::{Color, Stylize},
    widgets::{Block, Borders, Paragraph},
};
use ratatui_image::{
    picker::Picker,
    sliced::{SlicedImage, SlicedProtocol},
};

struct App {
    sliced: SlicedProtocol,
    size: Size,
    position: i16,
    background_text: Vec<String>,
    stopped: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut terminal = ratatui::init();

    let picker = Picker::from_query_stdio()?;
    let dyn_img = image::ImageReader::open("./assets/Ada.png")?.decode()?;
    let font_size = picker.font_size();
    let size = Size::new(
        dyn_img.width().div_ceil(font_size.0 as u32) as u16,
        dyn_img.height().div_ceil(font_size.1 as u32) as u16,
    );

    let mut terminal_size = Size::default();
    terminal.draw(|f| {
        terminal_size = f.area().into();
    })?;

    let mut background_text = format!(
        r#"Protocol: {:?}
font_size: {:?}
pixel size: {:?}
cols/rows: {:?}
terminal: {:?}
---------------
"#,
        picker.protocol_type(),
        picker.font_size(),
        dyn_img.dimensions(),
        (size.width, size.height),
        (terminal_size.width, terminal_size.height),
    );
    let source = fs::read_to_string("./examples/sliced.rs")?;
    background_text.push_str(&source);

    let sliced = SlicedProtocol::new(&picker, dyn_img, size)?;

    let mut app = App {
        sliced,
        size,
        position: -((size.height / 2) as i16),
        background_text: Vec::new(),
        stopped: false,
    };

    app.background_text = background_text.split("\n").map(|s| s.to_string()).collect();

    loop {
        let mut had_event = false;
        if ratatui::crossterm::event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => {
                            break;
                        }
                        KeyCode::Char('j') => {
                            app.stopped = true;
                            app.position += 1;
                            had_event = true;
                        }
                        KeyCode::Char('k') => {
                            app.stopped = true;
                            app.position -= 1;
                            had_event = true;
                        }
                        _ => {}
                    }
                }
            }
        }

        if app.stopped && !had_event {
            continue;
        }
        if !app.stopped {
            app.position += 1;
        }

        terminal.draw(|f| {
            let inner_height = f.area().height.saturating_sub(2) as i16;
            if app.position >= inner_height {
                app.position = -(app.size.height as i16);
            }
            if app.position < -(app.size.height as i16) {
                app.position = inner_height - 1;
            }

            ui(f, &app)
        })?;
    }

    ratatui::restore();

    Ok(())
}

fn ui(f: &mut Frame<'_>, app: &App) {
    let area = f.area();
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!("Scliced imaged on Y position {}", app.position))
        .bg(Color::Blue);
    let inner_area = block.inner(area);
    f.render_widget(block, area);

    for i in 0..f.area().height - 2 {
        if i as usize >= app.background_text.len() {
            break;
        }
        let p = Paragraph::new(app.background_text[i as usize].clone());
        f.render_widget(
            p,
            Rect::new(inner_area.x, inner_area.y + i, inner_area.width, 1),
        );
    }

    let mut area: Rect = app.size.into();
    area.x = 1;

    f.render_widget(
        SlicedImage::new(&app.sliced, app.size, app.position),
        inner_area,
    );
}
