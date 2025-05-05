mod error;
mod service;
pub use service::PdfService;
pub use error::Error;

#[derive(Clone, Copy)]
pub enum PageImageFormat
{
    Jpeg,
    Png,
    Webp
}