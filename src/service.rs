use std::{io::Cursor, path::{Path, PathBuf}, sync::Arc};
use crate::{Error, PageImageFormat};
use super::error;
use futures::{stream::FuturesOrdered, StreamExt};
use image::{DynamicImage, GrayImage, ImageFormat, RgbImage, RgbaImage};
use logger::error;
use tokio::runtime::Handle;
use pdfium_render::prelude::{PdfBitmapFormat, PdfPageRenderRotation, PdfRenderConfig, Pdfium};
//use pdfium_render::prelude::*;
pub struct PdfService 
{
    config: Arc<PdfRenderConfig>,
    path: PathBuf,
}
impl PdfService
{
    fn get_path(&self) -> &str
    {
        self.path.to_str().unwrap_or("")
    }
    pub fn new<P: AsRef<Path>>(path: P, w: i32, h: i32) -> Self
    {
        Self 
        { 
            config: Arc::new(PdfRenderConfig::new()
            .set_target_width(w)
            .set_maximum_height(h)
            .rotate_if_landscape(PdfPageRenderRotation::Degrees90, true)),
            path: path.as_ref().to_owned()
        }
    }
    fn get_instance() -> Result<Pdfium, error::Error> 
    {
        let dirs = ["./libs/", "libs/"];
        let binding_result = 
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(dirs[0]))
        .or_else(|_| Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(dirs[1])))
        .or_else(|_| Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(dirs[1])));
        if let Ok(result) = binding_result
        {
            return Ok(Pdfium::new(result));
        }
        else
        {
            let unrecognition_err : pdfium_render::prelude::PdfiumError = pdfium_render::prelude::PdfiumError::UnrecognizedPath;
            error!("библиотека pdfium не найдена в {} -> {}", dirs.join(","), unrecognition_err.to_string());
            return Err(error::Error::PdfiumError(unrecognition_err)); 
        }
       
    }
    
    ///Извлечение изображения из pdf и выдача в формате строки base64
    pub async fn convert_page(&self, page_number: u32, image_format: PageImageFormat) -> Result<Vec<u8>, error::Error> 
    {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let config = Arc::clone(&self.config);
        let path = self.path.clone();
        let path_str = self.get_path().to_owned();
        tokio::task::spawn_blocking(move ||
        {
            let pdfium = Self::get_instance();
            if pdfium.is_err()
            {
                let _ = sender.send(Err(pdfium.err().unwrap()));
                return;
            }
            let pdfium = pdfium.unwrap();
            let document = pdfium.load_pdf_from_file(&path, None);
            if document.is_err()
            {
                let _ = sender.send(Err(error::Error::PdfiumError(document.err().unwrap())));
                return;
            }
            let document = document.unwrap();
            let pages_count = document.pages().len();
            if page_number < 1 || page_number > pages_count as u32
            {
                let _ = sender.send(Err(error::Error::WrongPageSelect(path_str, pages_count as u32, page_number)));
                return;
            }
            let page_index = (page_number -1) as usize;
            let page = document.pages().iter().nth(page_index).unwrap();
            let current_page =  page.render_with_config(&config);
            if current_page.is_err()
            {
                let _ = sender.send(Err(error::Error::PdfiumError(current_page.err().unwrap())));
                return;
            }
            let current_page = current_page.unwrap();
            let bytes = current_page.as_rgba_bytes();
            let width = current_page.width() as u32;
            let height = current_page.height() as u32;
            let image = match current_page.format().unwrap_or_default() 
            {
                PdfBitmapFormat::Gray => 
                {
                    GrayImage::from_raw(width, height, bytes).map(DynamicImage::ImageLuma8)
                }
                _ => 
                {
                    match &image_format
                    {
                        PageImageFormat::Jpeg =>  RgbImage::from_raw(width, height, bytes).map(DynamicImage::ImageRgb8),
                        _ => RgbaImage::from_raw(width, height, bytes).map(DynamicImage::ImageRgba8)
                    }
                }
            };
            let _ = sender.send(image.ok_or(Error::ExtractDynamicImageError(path_str, page_number)));
        });

        if let Ok(page) = receiver.await
        {
            let image = page?;
            let png = self.gen_image(image, page_number, image_format).await?;
            return Ok(png);
        }
        else 
        {
            return Err(error::Error::ChannelError(self.get_path().to_owned()));
        }
    }

    pub async fn convert_all_pages(&self, image_format: PageImageFormat) -> Result<impl StreamExt<Item = Result<Vec<u8>, error::Error>>, error::Error>
    {
        let pages = Self::get_pages_count(&self.path).await?;
        let mut ordered = FuturesOrdered::new();
        for i in 1..=pages
        {
            ordered.push_back(Box::pin(self.convert_page(i as u32, image_format)));
        }
        //let futures: Vec<_> = (1..=pages).map(|i| Box::pin(self.convert_pdf_page_to_image(i as u32, image_format))).collect();
        //let stream = stream::iter(futures)
        //.buffered(2);
        Ok(ordered)
    }
    pub async fn convert_pages(&self, pages: &[u32], image_format: PageImageFormat) -> impl StreamExt<Item = Result<Vec<u8>, error::Error>>
    {
        let mut ordered = FuturesOrdered::new();
        for i in pages
        {
            ordered.push_back(Box::pin(self.convert_page(*i, image_format)));
        }
        ordered
    }
     ///Извлечение изображения из pdf
     pub async fn get_pages_count<P: AsRef<Path>>(path: P) -> Result<u16, error::Error> 
     {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let task_path = path.as_ref().to_owned();
        tokio::task::spawn_blocking(move ||
            {
                let pdfium = Self::get_instance();
                if pdfium.is_err()
                {
                    let _ = sender.send(Err(pdfium.err().unwrap()));
                    return;
                }
                let pdfium = pdfium.unwrap();
                let document = pdfium.load_pdf_from_file(&task_path, None);
                if document.is_err()
                {
                    let _ = sender.send(Err(error::Error::PdfiumError(document.err().unwrap())));
                    return;
                }
                let document = document.unwrap();
                let pages_count = document.pages().len();
                let _ = sender.send(Ok(pages_count));
            }
            
        );
        if let Ok(pages) = receiver.await
        {
            return pages;
        }
        else 
        {
            return Err(error::Error::ChannelError(path.as_ref().to_str().unwrap_or("").to_owned()));
        }
    }

    // Извлечение страницы из pdf и преобразование ее в формат rgba8 pdf и выдача страницы в виде массива байт
    async fn gen_image(&self, dyn_image: DynamicImage, page_number: u32, image_format: PageImageFormat) -> Result<Vec<u8>, error::Error>
    {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let current = Handle::current();
        let path = self.get_path().to_owned();
        tokio::task::spawn_blocking(move || 
        {
            current.block_on(
            async move
            {
                //бферизацию тут не используем, так как с io не работаем
                let mut buffer = std::io::BufWriter::new(Cursor::new(vec![]));
                //Изначально изображение в rgba поэтому для rgba берем as_rgba8 для остальных, например jpeg - dyn_image.to_rgb8()
                let converted = match image_format
                {
                    PageImageFormat::Png =>
                    {
                        if let Some(rgba8) = dyn_image.as_rgba8()
                        {
                            let res = rgba8.write_to(&mut buffer, ImageFormat::Png);
                            if res.is_err()
                            {
                                logger::error!("{}", res.as_ref().err().unwrap());
                                Err(error::Error::ImageConvertingError(page_number as u32, path.clone(), "png".to_owned()))
                            }
                            else 
                            {
                                Ok(())
                            }
                        }
                        else 
                        {
                            logger::error!("Изображение не может быть представлено как rgba8, необходима конвертация");
                            Err(error::Error::Rgba8ConvertError(path.clone(), page_number as u32))
                        }
                    }, 
                    PageImageFormat::Jpeg => 
                    {
                        if let Some(rgb) = dyn_image.as_rgb8()
                        {
                            let jpeg_quality = 90;
                            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buffer, jpeg_quality);
                            let res = rgb.write_with_encoder(encoder);
                            if res.is_err()
                            {
                                logger::error!("{}", res.as_ref().err().unwrap());
                                Err(error::Error::ImageConvertingError(page_number as u32, path.clone(), "jpeg".to_owned()))
                            }
                            else 
                            {
                                Ok(())
                            }
                        }
                        else 
                        {
                            logger::error!("Изображение не может быть представлено как rgb8, необходима конвертация");
                            Err(error::Error::Rgba8ConvertError(path.clone(), page_number as u32))
                        }
                        
                    },
                    PageImageFormat::Webp => 
                    {
                        if let Some(rgba) = dyn_image.as_rgba8()
                        {
                            let webp_encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut buffer);
                            let res = rgba.write_with_encoder(webp_encoder);
                            if res.is_err()
                            {
                                logger::error!("{}", res.as_ref().err().unwrap());
                                Err(error::Error::ImageConvertingError(page_number as u32, path.clone(), "webp".to_owned()))
                            }
                            else 
                            {
                                Ok(())
                            }
                        }
                        else 
                        {
                            logger::error!("Изображение не может быть представлено как rgb8, необходима конвертация");
                            Err(error::Error::Rgba8ConvertError(path.clone(), page_number as u32))
                        }
                    }
                };

                if let Ok(_) = converted
                {
                    let flash = buffer.into_inner();
                    if let Ok(f) = flash
                    {
                        let buff = f.into_inner();
                        let _ = sender.send(Ok(buff));
                    }
                    else 
                    {
                        let _ = sender.send(Err(error::Error::WriteBufferError(path.clone(), page_number as u32)));
                    }
                }
                else 
                {
                    let _ = sender.send(Err(converted.err().unwrap()));
                }
            })
        });
        if let Ok(image) = receiver.await
        {
            return image;
        }
        else 
        {
            return Err(error::Error::ChannelError(self.get_path().to_owned()));
        }
    }
}


#[cfg(test)]
mod async_tests
{

    use futures::{future::join_all, StreamExt};
    use logger::debug;

    use crate::PageImageFormat;

    #[tokio::test]
    async fn test_async_render()
    {
        let _ = logger::StructLogger::new_default();
        let path = "/home/phobos/Документы/Rust Language Cheat Sheet.pdf";
        let service = super::PdfService::new(path, 600, 800);
        let now = std::time::Instant::now();
        let futures: Vec<_> = (1..=29).map(|i| service.convert_page(i, PageImageFormat::Webp)).collect();
        let _ = join_all(futures).await;
        //let lenghts = r.iter().map(|f| f.as_ref().unwrap().len()).collect::<Vec<usize>>();
        //assert_eq!(&lenghts, &[194944, 230068, 227336, 229548, 243152, 240192, 227376, 244440, 223816, 213632, 219396, 251056, 249396, 231444, 240676, 251600, 274848, 245200, 216220]);
        debug!("Тестирование завершено за {}мc",  now.elapsed().as_millis());
    }
    #[tokio::test]
    async fn test_async_render_one()
    {
        let _ = logger::StructLogger::new_default();
        let path = "/home/phobos/Документы/ПОЧТА 14 04.04.2025 (отсортировано)/598-ПП.pdf";
        let service = super::PdfService::new(path, 600, 800);
        debug!("main: {:?}", std::thread::current().id());
        let now = std::time::Instant::now();
        let page = service.convert_page(1, PageImageFormat::Webp).await.unwrap();
        let _ = tokio::fs::write("page.webp", &page).await;
        debug!("Тестирование завершено за {}мc",  now.elapsed().as_millis());
    }
    #[tokio::test]
    async fn test_async_render_all()
    {
        let _ = logger::StructLogger::new_default();
        let path = "/home/phobos/Документы/Rust Language Cheat Sheet.pdf";
        let service = super::PdfService::new(path, 600, 800);
        let now = std::time::Instant::now();
        let mut stream = service.convert_all_pages(PageImageFormat::Webp).await.unwrap();
        while let Some(result) = stream.next().await 
        {
            match result 
            {
                Ok(val) => debug!("Успех: {}", val.len()),
                Err(e) => debug!("Ошибка: {}", e),
            }
        }
        //assert_eq!(5, pages.len());
        debug!("Тестирование завершено за {}мc",  now.elapsed().as_millis());
    }

    #[tokio::test]
    async fn test_async_render_pages()
    {
        let _ = logger::StructLogger::new_default();
        let path = "/home/phobos/Документы/Rust Language Cheat Sheet.pdf";
        let service = super::PdfService::new(path, 600, 800);
        let now = std::time::Instant::now();
        let mut stream = service.convert_pages(&[1,5,8,13], PageImageFormat::Webp).await;
        while let Some(result) = stream.next().await 
        {
            match result 
            {
                Ok(val) => debug!("Успех: {}", val.len()),
                Err(e) => debug!("{}", e),
            }
        }
        //assert_eq!(5, pages.len());
        debug!("Тестирование завершено за {}мc",  now.elapsed().as_millis());
    }
}