#[derive(Debug, thiserror::Error)]
pub enum Errors {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Image error: {0}")]
    ImageError(#[from] image::error::ImageError),
    #[error("Rustix error: {0}")]
    RustixError(#[from] rustix::io::Errno),
    #[error("{0}")]
    Str(&'static str),
    #[error("Sixel error: {0}")]
    CustomError(String),
}

impl From<&'static str> for Errors {
    fn from(s: &'static str) -> Self {
        Errors::Str(s)
    }
}

impl From<Box<dyn std::error::Error>> for Errors {
    fn from(e: Box<dyn std::error::Error>) -> Self {
        Errors::CustomError(e.to_string())
    }
}
