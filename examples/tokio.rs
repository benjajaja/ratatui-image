use color_eyre::eyre::Result;
use crossterm::event::{Event, EventStream};
use image::ImageReader;
use ratatui::{
    DefaultTerminal, Frame,
    widgets::{Block, Borders, Paragraph},
};
use ratatui_image::{
    StatefulImage,
    picker::Picker,
    thread::{ResizeRequest, ThreadProtocol},
};
use tokio::{
    select,
    sync::mpsc::{UnboundedReceiver, unbounded_channel},
};

use futures::{FutureExt, StreamExt};

struct App {
    running: bool,
    protocol: ThreadProtocol,
    event_stream: EventStream,
    rx: UnboundedReceiver<ResizeRequest>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let (tx, rx) = unbounded_channel();
    let protocol = Picker::from_query_stdio()?
        .new_resize_protocol(ImageReader::open("./assets/Ada.png")?.decode()?);
    App {
        protocol: ThreadProtocol::new(tx, Some(protocol)),
        event_stream: EventStream::new(),
        rx,
        running: true,
    }
    .run(ratatui::init())
    .await?;
    ratatui::restore();
    Ok(())
}

impl App {
    async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        while self.running {
            terminal.draw(|f| self.ui(f))?;
            select! {
                Some(event) = self.event_stream.next().fuse() => self.handle_event(event?),
                Some(request) = self.rx.recv() => self.handle_request(request)?,
            }
        }
        Ok(())
    }

    fn handle_event(&mut self, event: Event) {
        if let Event::Key(_) = event {
            self.running = false;
        }
    }

    fn handle_request(&mut self, request: ResizeRequest) -> Result<()> {
        self.protocol
            .update_resized_protocol(request.resize_encode()?);
        Ok(())
    }

    fn ui(&mut self, f: &mut Frame) {
        let area = f.area();
        let block = Block::default().borders(Borders::ALL).title("Async test");

        f.render_widget(
            Paragraph::new("PartiallyHiddenScreenshotParagraphBackground\n".repeat(10)),
            block.inner(area),
        );
        f.render_stateful_widget(StatefulImage::new(), block.inner(area), &mut self.protocol);
        f.render_widget(block, area)
    }
}
