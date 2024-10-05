use std::{
    assert_eq, env, io,
    process::{Command, Stdio},
};

use crossterm::{
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen, SetSize,
    },
};
use image::Rgb;
use ratatui::{
    backend::CrosstermBackend,
    layout::Rect,
    terminal::Frame,
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use ratatui_image::{picker::Picker, protocol::Protocol, Image, Resize};
struct App {
    image: Box<dyn Protocol>,
}

const ASSERT_FONT_SIZE: (u16, u16) = (9, 18);
const SCREEN_SIZE: (u16, u16) = (46, 12);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        SetSize(SCREEN_SIZE.0, SCREEN_SIZE.1)
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    #[cfg(all(feature = "rustix", unix))]
    let mut picker = Picker::from_termios()?;
    #[cfg(not(all(feature = "rustix", unix)))]
    let mut picker = {
        let font_size = (8, 16);
        Picker::new(font_size)
    };
    picker.guess_protocol();
    picker.background_color = Some(Rgb::<u8>([255, 0, 255]));
    if false {
        assert_eq!(
            ASSERT_FONT_SIZE, picker.font_size,
            "Font size must be fixed to a specific size: {ASSERT_FONT_SIZE:?}",
        );
    }
    let dyn_img = image::io::Reader::open("./assets/Ada.png")?.decode()?;
    let image = picker.new_protocol(
        dyn_img,
        Rect::new(0, 0, SCREEN_SIZE.0 - 10, SCREEN_SIZE.1 - 4),
        Resize::Fit(None),
    )?;
    let mut app = App { image };

    terminal.draw(|f| ui(f, &mut app))?;
    std::thread::sleep(std::time::Duration::from_secs(1)); // let the terminal actually draw.
    let xwd = Command::new("xwd")
        .args(["-root", "-silent"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start xwd command");
    let screenshot_term = env::var("SCREENSHOT_TERM_NAME").unwrap_or("unknown".to_string());
    std::process::Command::new("convert")
        .args([
            "xwd:-",
            &format!("png:./target/screenshot_{screenshot_term}.png"),
        ])
        .stdin(xwd.stdout.expect("failed to get stdout"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .and_then(|mut child| child.wait())?;

    // restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
    terminal.show_cursor()?;

    Ok(())
}

fn ui(f: &mut Frame<'_>, app: &mut App) {
    let area = Rect::new(0, 0, SCREEN_SIZE.0, SCREEN_SIZE.1);
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Screenshot test");

    f.render_widget(
        Paragraph::new("PartiallyHiddenScreenshotParagraphBackground\n".repeat(10)),
        block.inner(area),
    );

    let image = Image::new(app.image.as_ref());
    f.render_widget(image, block.inner(area));
    f.render_widget(block, area);
}
