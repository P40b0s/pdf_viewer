


#[derive(Debug, thiserror::Error)]
pub enum Error 
{
    #[error("Ошибка, в pdf {0} всего {1} страниц, а выбрана страница {2}")]
    WrongPageSelect(String, u32, u32),
    #[error("Для рендера pdf {0} не выбрано ни одной страницы")]
    NonePageSelect(String),
    #[error("Ошибка преобразования изображения в формат rgba8, файл: {0} страница {1}")]
    Rgba8ConvertError(String, u32),
    #[error("Ошибка записи в буфер, файл: {0} страница {1}")]
    WriteBufferError(String, u32),
    #[error("Ошибка получения изображения из канала сообщения для pdf {0}")]
    ChannelError(String),
    #[error(transparent)]
    PdfiumError(#[from] pdfium_render::prelude::PdfiumError),
    #[error(transparent)]
    ImageError(#[from] image::ImageError),
    #[error("Ошибка создание изображения из файла: {0} страницы {1}")]
    ExtractDynamicImageError(String, u32),
    //Ошибка если дата и размер копируемого файла не может синхронизироваться больше 2 минут
    #[error("Превышено максимальное количество попыток при попытке копирования файла `{0}`, файл должен успевать копироваться в систему в течении 2 минут")]
    FileTimeCopyError(String),
    #[error("Ошибка конвертирование страницы {0} файла {1} в формат {2}")]
    ImageConvertingError(u32, String, String),
}
impl serde::Serialize for Error 
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
    S: serde::ser::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}