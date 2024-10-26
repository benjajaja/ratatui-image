use std::sync::mpsc::RecvTimeoutError;

#[derive(Debug, thiserror::Error)]
pub enum Errors {
    #[error("Could not detect font size")]
    NoFontSize,
    #[error("Could not detect any graphics capabilities")]
    NoCap,
    #[error("Timeout: {0}")]
    Timeout(#[from] RecvTimeoutError),
    #[error("Sixel error: {0}")]
    Sixel(String),
    #[error("Tmux error: {0}")]
    Tmux(&'static str),
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Image error: {0}")]
    Image(#[from] image::error::ImageError),
}

#[cfg(not(windows))]
impl From<rustix::io::Errno> for Errors {
    fn from(errno: rustix::io::Errno) -> Self {
        Errors::IO(std::io::Error::from(errno))
    }
}

#[cfg(windows)]
impl From<windows::core::Error> for Errors {
    fn from(err: windows::core::Error) -> Self {
        Errors::IO(std::io::Error::new(
            std::io::ErrorKind::Other,
            err.to_string(),
        ))
    }
}
