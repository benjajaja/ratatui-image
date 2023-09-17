# ratatui-image

[![GitHub CI
Status](https://img.shields.io/github/actions/workflow/status/benjajaja/ratatui-image/ci.yaml?style=flat-square&logo=github)](https://github.com/benjajaja/ratatui-image/actions?query=workflow%3A)

### Showcase:

![Recording](./assets/Recording.gif)

Image widgets for [Ratatui]

**⚠️ THIS CRATE IS EXPERIMENTAL**

**⚠️ THE `TERMWIZ` RATATUI BACKEND IS BROKEN WITH THIS CRATE**

Render images with graphics protocols in the terminal with [Ratatui].

## Quick start
```rust
use ratatui::{backend::{Backend, TestBackend}, Terminal, terminal::Frame, layout::Rect};
use ratatui_image::{
  picker::{Picker, ProtocolType},
  ImageSource, Resize, ResizeImage, protocol::ResizeProtocol,
};

struct App {
    // We need to hold the render state.
    image: Box<dyn ResizeProtocol>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // It is highly recommended to use Picker::from_termios() instead!
    let mut picker = Picker::new((7, 16), ProtocolType::Sixel, None)?;

    let dyn_img = image::io::Reader::open("./assets/Ada.png")?.decode()?;
    let image = picker.new_state(dyn_img);
    let mut app = App { image };

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend)?;

    // loop:
    terminal.draw(|f| ui(f, &mut app))?;

    Ok(())
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let image = ResizeImage::new(None);
    f.render_stateful_widget(image, f.size(), &mut app.image);
}
```

## Examples

See the [crate::picker::Picker] helper and [`examples/demo`](./examples/demo/main.rs).
The lib also includes a binary that renders an image file.

[Ratatui]: https://github.com/ratatui-org/ratatui
[Sixel]: https://en.wikipedia.org/wiki/Sixel
[Ratatui PR for cell skipping]: https://github.com/ratatui-org/ratatui/pull/215
[Ratatui PR for getting window size]: https://github.com/ratatui-org/ratatui/pull/276

Current version: 0.2.0

Sixel compatibility and QA:

Terminal   | Fixed | Resize | Notes
-----------|-------|--------|-------
Xterm      | ✔️     | ✔️      |
Foot       | ✔️     | ✔️      |
kitty      | ✔️     | ✔️      |
Alacritty  | ✔️     | ❌     | [with sixel patch](https://github.com/microo8/alacritty-sixel), but never clears graphics.
iTerm2     | ❌    | ❌     | Unimplemented, has a protocolo [similar to sixel](https://iterm2.com/documentation-images.html)
konsole    | ❌    | ❌     | Does not clear graphics unless cells have a background style
Contour    | ❌    | ❌     | Text over graphics
Wezterm    | ❌    | ❌     | [Buggy](https://github.com/wez/wezterm/issues/217#issuecomment-1657075311)
ctx        | ❌    | ❌     | Buggy
Blackbox   | ❔    | ❔     | Untested

Latest Xterm testing screenshot:  
![Testing screenshot](./assets/test_screenshot.png)

Halfblocks should work in all terminals.

### Comparison:

* [viuer](https://crates.io/crates/viuer)
  Renders graphics in different terminals/protocols, but "dumps" the image, making it difficult to
  work for TUI programs.
  The terminal protocol guessing code has been adapted to rustix, thus the author of viuer is
  included in the copyright notice.
* [yazi](https://github.com/sxyazi/yazi)
  Not a library but a terminal file manager that implementes many graphics protocols and lets you
  preview images in the filesystem.

License: MIT
