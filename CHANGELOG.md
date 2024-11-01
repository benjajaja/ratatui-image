# Changelog

All notable changes to this project will be documented in this file.

# [3.0.0] - 2024-11-11

Compatible with `ratatui`: `v0.29.0`.

### Windows support

Thanks to Terakomari <im.terakomari@icloud.com>, windows with WSL is now supported.

The latest terminal supports sixels and necessary CSI sequences.
The CI now runs (almost) fully on Windows.

### Font size guessing and CSI parsing

Font size is queried with CSI sequence instead of tcgetattr.
The CSI response parsing has been extended into a full parser module.

### Miscellaneous improvements

- `rustix` is only used on non-windows, is not a crate feature anymore
- Improve magic env var guessing
- Kitty resets only background color
- iTerm2 use PNG intermediate encoding instead of Jpeg
- Use direnv with flake for development

### Static dispatch

Inspired by Uttarayan Mondal <email@uttarayan.me>, changed to static dispatch with an enum instead of a `Box<dyn Protocol>`.
Adding a custom protocol isn't really a use case. Errors are also static now instead of `Box<dyn Error>`.

# [2.0.1] - 2024-10-07

(See git log)
