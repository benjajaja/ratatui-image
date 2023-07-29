# {{crate}}

### Showcase:

![Recording](./assets/Recording.gif)

{{readme}}

Current version: {{version}}

Sixel compatibility and QA:

Terminal   | Fixed | Resize | Notes
-----------|-------|--------|-------
Xterm      | ✅    | ✅     |
Foot       | ✅    | ✅     |
kitty      | 😸    | 😸     | Has it own protocol which should be implemented here (WIP)
Alacritty  | ✅    | ❌     | [with sixel patch](https://github.com/microo8/alacritty-sixel), never clears graphics.
konsole    | ❗    | ❗     | Does not clear graphics unless cells have a background style
Contour    | ❗    | ❗     | Text over graphics
Wezterm    | ❌    | ❌     | [Buggy](https://github.com/wez/wezterm/issues/217#issuecomment-1657075311)
ctx        | ❌    | ❌     | Buggy
Blackbox   | ❔    | ❔     | Untested
iTerm2     | ❔    | ❔     | Untested

Latest Xterm testing screenshot:  
![Testing screenshot](./assets/test_screenshot.png)

Halfblocks should work in all terminals.

License: {{license}}

