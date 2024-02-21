# Ratatui-image

[![CI Badge]][CI]
[![Crate Badge]][Crate]
[![Docs Badge]][Docs]

[CI Badge]: https://img.shields.io/github/actions/workflow/status/benjajaja/ratatui-image/ci.yaml?style=flat-square&logo=github
[CI]: https://github.com/benjajaja/ratatui-image/actions?query=workflow%3A
[Crate Badge]: https://img.shields.io/crates/v/ratatui-image?logo=rust&style=flat-square
[Crate]: https://crates.io/crates/ratatui-image
[Docs Badge]: https://img.shields.io/docsrs/ratatui-image?logo=rust&style=flat-square
[Docs]: https://docs.rs/ratatui-image/0.8.0/ratatui_image/

### Showcase:

![Recording](./assets/Recording.gif)

{{readme}}

### Compatibility matrix

Compatibility and QA:

Terminal  | Protocol | Fixed | Resize | Notes
----------|----------|-------|--------|-------
Xterm     | `Sixel`  | ✔️     | ✔️      | Run with `-ti 340` to make sure sixel support is enabled.
Foot      | `Sixel`  | ✔️     | ✔️      | Wayland.
kitty     | `Kitty`  | ✔️     | ✔️      |
Wezterm   | `iTerm2` | ✔️     | ✔️      | Also would support `Sixel` and `Kitty`, but only `iTerm2` actually works bug-free.
Alacritty | `Sixel`  | ✔️     | ❌     | [only with sixel patch](https://github.com/microo8/alacritty-sixel), but does not clear graphics. Will not be merged in the foreseeable future.
iTerm2    | `iTerm2` | ❔    | ❔     | Untested (needs apple hardware), however should be the same as WezTerm.
konsole   | `Sixel`  | ❌    | ❌     | [Wontfix: does not clear graphics](https://bugs.kde.org/show_bug.cgi?id=456354), other artifacts.
Contour   | `Sixel`  | ❌    | ❌     | Does not clear graphics.
ctx       | `Sixel`  | ❌    | ❌     | Buggy.
Blackbox  | `Sixel`  | ❔    | ❔     | Untested.

Here, "Fixed" means the `Image` widget, and "Resize" is the `StatefulWidget`.

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
* [Überzug++](https://github.com/jstkdng/ueberzugpp)
  CLI utility that draws images on terminals by using X11/wayland child windows, sixels, kitty,
  and/or iterm2 protocols (any means necessary). There exists several wrapper or bindings crates.
  More battle-tested but essentially stateful, which makes it hard to use with immediate-mode.

License: {{license}}

