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
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};
use ratatui_image::{
    backend::{pick_resizeable, pick_static, DynamicBackend, StaticBackend},
    ImageSource, ResizableImage, Resize, StaticImage,
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

    pub image_static: Box<dyn StaticBackend>,

    pub image_source: ImageSource,
    pub image_fit_state: Box<dyn DynamicBackend>,
    pub image_crop_state: Box<dyn DynamicBackend>,
}

impl<'a> App<'a> {
    pub fn new<B: Backend>(title: &'a str, terminal: &mut Terminal<B>) -> App<'a> {
        let dyn_img = image::io::Reader::open("./assets/Ada.png")
            .unwrap()
            .decode()
            .unwrap();

        let image_static = pick_static(dyn_img.clone(), terminal).unwrap();

        let (image_scale, image_fit_state) = pick_resizeable(dyn_img, terminal).unwrap();
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
            image_static,
            image_source: image_scale,
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
            'h' => {
                if self.split_percent >= 10 {
                    self.split_percent -= 10;
                }
            }
            'l' => {
                if self.split_percent <= 90 {
                    self.split_percent += 10;
                }
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

    let block_left_top = Block::default().borders(Borders::ALL).title("Static");
    let area = block_left_top.inner(left_chunks[0]);
    f.render_widget(
        Paragraph::new(app.background.as_str()).wrap(Wrap { trim: true }),
        area,
    );
    f.render_widget(block_left_top, left_chunks[0]);
    let image = StaticImage::new(app.image_static.as_ref());
    f.render_widget(image, area);

    let block_left_bottom = Block::default()
        .borders(Borders::ALL)
        .title("Fit into area");
    if app.show_resizable_images {
        let image = ResizableImage::new(&app.image_source).resize(Resize::Fit);
        f.render_stateful_widget(
            image,
            block_left_bottom.inner(left_chunks[1]),
            &mut app.image_fit_state,
        );
    }
    f.render_widget(block_left_bottom, left_chunks[1]);

    let block_right_top = Block::default().borders(Borders::ALL).title("Fit");
    if app.show_resizable_images {
        let image = ResizableImage::new(&app.image_source).resize(Resize::Crop);
        f.render_stateful_widget(
            image,
            block_right_top.inner(right_chunks[0]),
            &mut app.image_crop_state,
        );
    }
    f.render_widget(block_right_top, right_chunks[0]);
}
