#[cfg(all(
    not(feature = "crossterm"),
    not(feature = "termion"),
    not(feature = "termwiz")
))]
compile_error!("The demo needs one of the crossterm, termion, or termwiz features");
#[cfg(not(feature = "rustix"))]
compile_error!("The demo needs rustix until window_size is on ratataui");

#[cfg(feature = "crossterm")]
mod crossterm;
#[cfg(feature = "termion")]
mod termion;
#[cfg(feature = "termwiz")]
mod termwiz;

use std::{error::Error, path::PathBuf, time::Duration};

use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};
use ratatui_image::{
    picker::{BackendType, Picker},
    protocol::{Protocol, ResizeProtocol},
    FixedImage, ImageSource, Resize, ResizeImage,
};

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(feature = "crossterm")]
    crate::crossterm::run()?;
    #[cfg(feature = "termion")]
    crate::termion::run()?;
    #[cfg(feature = "termwiz")]
    crate::termwiz::run()?;
    Ok(())
}

#[derive(Debug)]
enum ShowImages {
    All,
    Fixed,
    Resized,
}

struct App<'a> {
    pub title: &'a str,
    pub should_quit: bool,
    pub tick_rate: Duration,
    pub background: String,
    pub split_percent: u16,
    pub show_images: ShowImages,

    pub image_source_path: PathBuf,
    pub image_static_offset: (u16, u16),

    pub picker: Picker,
    pub image_source: ImageSource,
    pub image_static: Box<dyn Protocol>,
    pub image_fit_state: Box<dyn ResizeProtocol>,
    pub image_crop_state: Box<dyn ResizeProtocol>,
}

fn size() -> Rect {
    Rect::new(0, 0, 30, 16)
}

impl<'a> App<'a> {
    pub fn new<B: Backend>(title: &'a str, _: &mut Terminal<B>) -> App<'a> {
        let ada = "./assets/Ada.png";
        let dyn_img = image::io::Reader::open(ada).unwrap().decode().unwrap();

        let mut picker = Picker::from_termios(None).unwrap();

        let image_static = picker
            .new_static_fit(dyn_img.clone(), size(), Resize::Fit)
            .unwrap();

        let image_source = ImageSource::new(dyn_img.clone(), picker.font_size());
        let image_fit_state = picker.new_state(dyn_img.clone());
        let image_crop_state = picker.new_state(dyn_img);

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
            show_images: ShowImages::All,
            split_percent: 70,
            picker,
            image_source,
            image_source_path: ada.into(),

            image_static,
            image_fit_state,
            image_crop_state,

            image_static_offset: (0, 0),
        }
    }
    pub fn on_key(&mut self, c: char) {
        match c {
            'q' => {
                self.should_quit = true;
            }
            't' => {
                self.show_images = match self.show_images {
                    ShowImages::All => ShowImages::Fixed,
                    ShowImages::Fixed => ShowImages::Resized,
                    ShowImages::Resized => ShowImages::All,
                }
            }
            'i' => {
                let next = match self.picker.backend_type() {
                    #[cfg(not(feature = "sixel"))]
                    BackendType::Halfblocks => BackendType::Kitty,
                    #[cfg(feature = "sixel")]
                    BackendType::Halfblocks => BackendType::Sixel,
                    #[cfg(feature = "sixel")]
                    BackendType::Sixel => BackendType::Kitty,
                    BackendType::Kitty => BackendType::Halfblocks,
                };
                self.picker.set(next);

                self.image_static = self
                    .picker
                    .new_static_fit(self.image_source.image.clone(), size(), Resize::Fit)
                    .unwrap();

                self.image_fit_state.reset();
                self.image_crop_state.reset();
            }
            'o' => {
                let path = match self.image_source_path.to_str() {
                    Some("./assets/Ada.png") => "./assets/Jenkins.jpg",
                    _ => "./assets/Ada.png",
                };
                let dyn_img = image::io::Reader::open(path).unwrap().decode().unwrap();
                self.image_source = ImageSource::new(dyn_img.clone(), self.picker.font_size());

                self.image_static = self
                    .picker
                    .new_static_fit(dyn_img, size(), Resize::Fit)
                    .unwrap();
                self.image_source_path = path.into();
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

fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
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
    match app.show_images {
        ShowImages::Resized => {}
        _ => {
            let image = FixedImage::new(app.image_static.as_ref());
            f.render_widget(image, area);
        }
    }

    let chunks_left_bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(left_chunks[1]);

    let block_left_bottom = Block::default().borders(Borders::ALL).title("Crop");
    let area = block_left_bottom.inner(chunks_left_bottom[0]);
    f.render_widget(
        Paragraph::new(app.background.as_str()).wrap(Wrap { trim: true }),
        area,
    );
    match app.show_images {
        ShowImages::Fixed => {}
        _ => {
            let image = ResizeImage::new(None).resize(Resize::Crop);
            f.render_stateful_widget(
                image,
                block_left_bottom.inner(chunks_left_bottom[0]),
                &mut app.image_crop_state,
            );
        }
    }
    f.render_widget(block_left_bottom, chunks_left_bottom[0]);

    let block_middle_bottom = Block::default().borders(Borders::ALL).title("Placeholder");
    f.render_widget(
        Paragraph::new(app.background.as_str())
            .wrap(Wrap { trim: true })
            .style(Style::new().bg(Color::Blue)),
        block_middle_bottom.inner(chunks_left_bottom[1]),
    );
    f.render_widget(block_middle_bottom, chunks_left_bottom[1]);

    let block_right_top = Block::default().borders(Borders::ALL).title("Fit");
    let area = block_right_top.inner(right_chunks[0]);
    f.render_widget(
        Paragraph::new(app.background.as_str()).wrap(Wrap { trim: true }),
        area,
    );
    match app.show_images {
        ShowImages::Fixed => {}
        _ => {
            let image = ResizeImage::new(None).resize(Resize::Fit);
            f.render_stateful_widget(
                image,
                block_right_top.inner(right_chunks[0]),
                &mut app.image_fit_state,
            );
        }
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
                app.picker.backend_type()
            )),
            Line::from("o: cycle image"),
            Line::from(format!("t: toggle ({:?})", app.show_images)),
            Line::from(format!("Font size: {:?}", app.picker.font_size())),
        ])
        .wrap(Wrap { trim: true }),
        area,
    );
    f.render_widget(block_right_bottom, right_chunks[1]);
}
