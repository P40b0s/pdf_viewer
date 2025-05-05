## Async library for render pdf page to image (based on pdfium_render)  
#### One page 
```rust
 let service = PdfService::new(path, 600, 800);
  let image: Vec<u8> = service.convert_page(1, PageImageFormat::Webp).await.unwrap();
```
#### All pages
```rust
let service = PdfService::new(path, 600, 800);
let mut stream = service.convert_all_pages(PageImageFormat::Webp).await.unwrap();
while let Some(result) = stream.next().await 
{
    match result 
    {
        Ok(val) => println!("Ok: {}", val.len()),
        Err(e) => println!("Error: {}", e),
    }
}
```
#### Multiple pages
```rust
let service = PdfService::new(path, 600, 800);
let mut stream = service.convert_pages(&[1,5,8,13], PageImageFormat::Webp).await;
while let Some(result) = stream.next().await 
{
    match result 
    {
        Ok(val) => debug!("Ok: {}", val.len()),
        Err(e) => debug!("Error {}", e),
    }
}
```