#[cfg(all(
    not(feature = "crossterm"),
    not(feature = "termion"),
    not(feature = "termwiz")
))]
compile_error!("The demo needs one of the crossterm, termion, or termwiz features");

#[cfg(feature = "crossterm")]
mod crossterm;
#[cfg(feature = "termion")]
mod termion;
#[cfg(feature = "termwiz")]
mod termwiz;

use std::{error::Error, time::Duration};

use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};
use ratatui_imagine::{
    backend::{FixedBackend, ResizeBackend},
    picker::Picker,
    FixedImage, Resize, ResizeImage,
};

#[cfg(feature = "crossterm")]
use crate::crossterm::run;
#[cfg(feature = "termion")]
use crate::termion::run;
#[cfg(feature = "termwiz")]
use crate::termwiz::run;

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(any(feature = "crossterm", feature = "termion", feature = "termwiz"))]
    run()?;
    Ok(())
}

pub struct App<'a> {
    pub title: &'a str,
    pub should_quit: bool,
    pub tick_rate: Duration,
    pub background: String,
    pub split_percent: u16,
    pub show_resizable_images: bool,

    pub picker: Picker,

    pub image_static: Box<dyn FixedBackend>,
    pub image_static_offset: (u16, u16),

    pub image_fit_state: Box<dyn ResizeBackend>,
    pub image_crop_state: Box<dyn ResizeBackend>,
}

// Since terminal cell proportion is around 1:2, this roughly results in a "portrait" proportion.
fn static_fit() -> Rect {
    Rect::new(0, 0, 30, 20)
}

impl<'a> App<'a> {
    pub fn new<B: Backend>(title: &'a str, terminal: &mut Terminal<B>) -> App<'a> {
        let dyn_img = image::io::Reader::open("./assets/Ada.png")
            .unwrap()
            .decode()
            .unwrap();

        let picker = Picker::guess(dyn_img, terminal, None).unwrap();

        let image_static = picker.new_static_fit(Resize::Fit, static_fit()).unwrap();

        let image_fit_state = picker.new_state();
        let image_crop_state = image_fit_state.clone();

        let mut background = String::new();

        for i in 0..10_000 {
            let c: char = ((48 + (i % 70)) as u8).into();
            background.push(c);
        }

        App {
            title,
            should_quit: false,
            tick_rate: Duration::from_millis(1000),
            background,
            show_resizable_images: true,
            split_percent: 70,
            picker,
            image_static,
            image_static_offset: (0, 0),
            image_fit_state,
            image_crop_state,
        }
    }
    pub fn on_key(&mut self, c: char) {
        match c {
            'q' => {
                self.should_quit = true;
            }
            't' => {
                self.show_resizable_images = !self.show_resizable_images;
            }
            'i' => {
                self.picker.next();
                self.image_static = self
                    .picker
                    .new_static_fit(Resize::Fit, static_fit())
                    .unwrap();
                self.image_fit_state = self.picker.new_state();
                self.image_crop_state = self.picker.new_state();
            }
            'H' => {
                if self.split_percent >= 10 {
                    self.split_percent -= 10;
                }
            }
            'L' => {
                if self.split_percent <= 90 {
                    self.split_percent += 10;
                }
            }
            'h' => {
                if self.image_static_offset.0 > 0 {
                    self.image_static_offset.0 -= 1;
                }
            }
            'j' => {
                self.image_static_offset.1 += 1;
            }
            'k' => {
                if self.image_static_offset.1 > 0 {
                    self.image_static_offset.1 -= 1;
                }
            }
            'l' => {
                self.image_static_offset.0 += 1;
            }
            _ => {}
        }
    }

    pub fn on_tick(&mut self) {}
}

pub fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let outer_block = Block::default().borders(Borders::TOP).title(app.title);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(app.split_percent),
                Constraint::Percentage(100 - app.split_percent),
            ]
            .as_ref(),
        )
        .split(outer_block.inner(f.size()));
    f.render_widget(outer_block, f.size());

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(chunks[0]);
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(chunks[1]);

    let block_left_top = Block::default().borders(Borders::ALL).title("Fixed");
    let area = block_left_top.inner(left_chunks[0]);
    f.render_widget(
        Paragraph::new(app.background.as_str()).wrap(Wrap { trim: true }),
        area,
    );
    f.render_widget(block_left_top, left_chunks[0]);
    let image = FixedImage::new(app.image_static.as_ref());
    f.render_widget(image, area);

    let block_left_bottom = Block::default().borders(Borders::ALL).title("Crop");
    let area = block_left_bottom.inner(left_chunks[1]);
    f.render_widget(
        Paragraph::new(app.background.as_str()).wrap(Wrap { trim: true }),
        area,
    );
    if app.show_resizable_images {
        let image = ResizeImage::new(&app.picker.source).resize(Resize::Crop);
        f.render_stateful_widget(
            image,
            block_left_bottom.inner(left_chunks[1]),
            &mut app.image_fit_state,
        );
    }
    f.render_widget(block_left_bottom, left_chunks[1]);

    let block_right_top = Block::default().borders(Borders::ALL).title("Fit");
    let area = block_right_top.inner(right_chunks[0]);
    f.render_widget(
        Paragraph::new(app.background.as_str()).wrap(Wrap { trim: true }),
        area,
    );
    if app.show_resizable_images {
        let image = ResizeImage::new(&app.picker.source).resize(Resize::Fit);
        f.render_stateful_widget(
            image,
            block_right_top.inner(right_chunks[0]),
            &mut app.image_crop_state,
        );
    }
    f.render_widget(block_right_top, right_chunks[0]);

    let block_right_bottom = Block::default().borders(Borders::ALL).title("Help");
    let area = block_right_bottom.inner(right_chunks[1]);
    f.render_widget(
        Paragraph::new(vec![
            Line::from("Key bindings:"),
            Line::from("H/L: resize"),
            Line::from(format!(
                "i: cycle image backends (current: {:?})",
                app.picker.current()
            )),
            Line::from("t: toggle rendering dynamic image widgets"),
            Line::from(format!("Font size: {:?}", app.picker.source.font_size)),
        ])
        .wrap(Wrap { trim: true }),
        area,
    );
    f.render_widget(block_right_bottom, right_chunks[1]);
}
