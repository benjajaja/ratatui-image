# ratatu-image

[![GitHub CI
Status](https://img.shields.io/github/actions/workflow/status/benjajaja/ratatu-image/ci.yml?style=flat-square&logo=github)](https://github.com/benjajaja/ratatu-image/actions?query=workflow%3ACI+)

### Showcase:

![Recording](./assets/Recording.gif)

Image widgets for [Ratatui]

**⚠️ THIS CRATE IS EXPERIMENTAL**

Render images with graphics protocols in the terminal with [Ratatui].

```rust
struct App {
    image: Box<dyn FixedBackend>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let font_size = (7, 16); // Or use Picker::from_termios, or let user provide it.
    let mut picker = Picker::new(
        font_size,
        BackendType::Sixel,
        None,
    )?;
    let dyn_img = image::io::Reader::open("./assets/Ada.png")?.decode()?;
    let image = picker.new_static_fit(dyn_img, Rect::new(0, 0, 30, 20), Resize::Fit)?;
    let mut app = App { image };

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend)?;

    // loop:
    terminal.draw(|f| ui(f, &mut app))?;

    Ok(())
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let image = FixedImage::new(app.image.as_ref());
    f.render_widget(image, f.size());
}
```

## TUIs
TUI application revolve around columns and rows of text naturally without the need of any
notions of pixel sizes. [Ratatui] is based on "immediate rendering with intermediate buffers".

At each frame, widgets are constructed and rendered into some character buffer, and any changes
from respect to the last frame are then diffed and written to the terminal screen.

## Terminal graphic protocols
Some protocols allow to output image data to terminals that support it.

The [Sixel] protocol mechanism is, in a nutshell, just printing an escape sequence.
The image will be "dumped" at the cursor position, and the implementation may add enough
carriage returns to scroll the output.

## Problem
Simply "dumping" an image into a [Ratatui] buffer is not enough. At best, the buffer diff might
not overwrite any characters that are covered by the image in some instances, but the diff
might change at any time due to screen/area resizing or simply other widget's contents
changing. Then the graphics would inmediately get overwritten by the underlying character data.

## Solution
First it is necessary to suppress the covered character cells' rendering, which is addressed in
a [Ratatui PR for cell skipping].

Second it is then necessary to get the image's size in columns and rows, which is done by
querying the terminal for it's pixel size and dividing by columns/rows to get the font size in
pixels. Currently this is implemented with `rustix::termios`, but this is subject to change for
a [Ratatui PR for getting window size].

## Implementation

The images are always resized so that they fit their nearest rectangle in columns/rows.
This is so that the image shall be drawn in the same "render pass" as all surrounding text, and
cells under the area of the image skip the draw on the ratatui buffer level, so there is no way
to "clear" previous drawn text. This would leave artifacts around the image's right and bottom
borders.

## Example

See the [crate::picker::Picker] helper and [`examples/demo`](./examples/demo/main.rs).

[Ratatui]: https://github.com/ratatui-org/ratatui
[Sixel]: https://en.wikipedia.org/wiki/Sixel
[Ratatui PR for cell skipping]: https://github.com/ratatui-org/ratatui/pull/215
[Ratatui PR for getting window size]: https://github.com/ratatui-org/ratatui/pull/276

Current version: 0.1.1

Sixel compatibility and QA:

Terminal   | Fixed | Resize | Notes
-----------|-------|--------|-------
Xterm      | ✔️     | ✔️      |
Foot       | ✔️     | ✔️      |
kitty      | ✔️     | ✔️      |
Alacritty  | ✔️     | ❌     | [with sixel patch](https://github.com/microo8/alacritty-sixel), never clears graphics.
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

License: MIT
