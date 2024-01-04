# ratatui-image

[![GitHub CI
Status](https://img.shields.io/github/actions/workflow/status/benjajaja/ratatui-image/ci.yaml?style=flat-square&logo=github)](https://github.com/benjajaja/ratatui-image/actions?query=workflow%3A)

### Showcase:

![Recording](./assets/Recording.gif)

## Image widgets with multiple graphics protocol backends for [Ratatui]
[Ratatui] is an immediate-mode TUI library that does 3 things:

1. **Query the terminal for available graphics protocols** (or guess from `$TERM` or similar).
Some terminals may implement one or more graphics protocols, such as Sixels or Kitty's
graphics protocol. Query the terminal with some escape sequence. Fallback to "halfblocks" which
uses some unicode half-block characters with fore- and background colors.

2. **Query the terminal for the font-size in pixels.**
If there is an actual graphics protocol available, it is necessary to know the font-size to
be able to map the image pixels to character cell area. The image can be resized, fit, or
cropped to an area. Query the terminal for the window and columns/rows sizes, and derive the
font-size.

3. **Render the image by the means of the guessed protocol.**
Some protocols, like Sixels, are essentially "immediate-mode", but we still need to avoid the
TUI from overwriting the image area, even with blank characters.

Other protocols, like Kitty, are essentially stateful, but at least provide a way to re-render
an image that has been loaded, at a different or same position.

## Quick start
```rust
use ratatui::{backend::{Backend, TestBackend}, Terminal, terminal::Frame};
use ratatui_image::{picker::Picker, StatefulImage, protocol::StatefulProtocol};

struct App {
    // We need to hold the render state.
    image: Box<dyn StatefulProtocol>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend)?;

    // Should use Picker::from_termios(), but we can't put that here because that would break doctests!
    let mut picker = Picker::new((8, 12));
    picker.guess_protocol();

    let dyn_img = image::io::Reader::open("./assets/Ada.png")?.decode()?;
    let image = picker.new_resize_protocol(dyn_img);
    let mut app = App { image };

    // This would be your typical `loop {` in a real app:
    terminal.draw(|f| ui(f, &mut app))?;

    Ok(())
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let image = StatefulImage::new(None);
    f.render_stateful_widget(image, f.size(), &mut app.image);
}
```

## Graphics protocols in terminals
Different terminals support different graphics protocols such as sixels,
kitty-graphics-protocol, or iTerm2-graphics-protocol. If no such protocol is supported, it is
still possible to render images with unicode "halfblocks" that have fore- and background color.

The [picker::Picker] helper is there to do all this graphics-protocol guessing, and also to map
character-cell-size to pixel size so that we can e.g. "fit" an image inside a desired
columns+rows bound etc.

## Widget choice
* The [Image] widget does not adapt to rendering area (except not drawing at all if space
is insufficient), may be a bit more bug prone (overdrawing or artifacts), and is not friendly
with some of the protocols (e.g. the Kitty graphics protocol, which is stateful). Its big
upside is that it is _stateless_ (in terms of ratatui, i.e. immediate-mode), and thus can never
block the rendering thread/task. A lot of ratatui apps only use stateless widgets.
* The [StatefulImage] widget adapts to its render area, is more robust against overdraw bugs and
artifacts, and plays nicer with some of the graphics protocols.
The resizing and encoding is blocking by default, but it is possible to offload this to another
thread or async task (see `examples/async.rs`). It must be rendered with
[`render_stateful_widget`] (i.e. with some mutable state).

## Examples

`examples/demo.rs` is a fully fledged demo:
* Guessing the graphics protocol and the terminal font-size.
* Both [Image] and [StatefulImage].
* [Resize::Fit] and [Resize::Crop].
* Reacts to resizes from terminal or widget layout.
* Cycle through available graphics protocols at runtime.
* Load different images.
* Cycle toggling [Image], [StatefulImage], or both, to demonstrate correct state after
  removal.
* Works with crossterm and termion backends.

`examples/async.rs` shows how to offload resize and encoding to another thread, to avoid
blocking the UI thread.

The lib also includes a binary that renders an image file.

## Features
* `sixel` (default) compiles with libsixel.
* `rustix` (default) enables much better guessing of graphics protocols with `rustix::termios::tcgetattr`.
* `crossterm` or `termion` should match your ratatui backend. `termwiz` is available, but not
working correctly with ratatu-image.
* `serde` for `#[derive]`s on [picker::ProtocolType] for convenience, because it might be
useful to save it in some user configuration.

[Ratatui]: https://github.com/ratatui-org/ratatui
[Sixel]: https://en.wikipedia.org/wiki/Sixel
[`render_stateful_widget`]: https://docs.rs/ratatui/latest/ratatui/terminal/struct.Frame.html#method.render_stateful_widget

Current version: 0.5.0

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
