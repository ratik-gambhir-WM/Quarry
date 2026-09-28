use super::*;
use image::{DynamicImage, ImageOutputFormat, RgbaImage};
use std::io::Cursor;

#[tokio::test]
async fn validates_a_supported_image_type_from_the_filename() {
    assert_eq!(
        validate_image_by_bytes(png_bytes(1, 1), "screenshot.PNG")
            .await
            .unwrap()
            .mime_type,
        "image/png"
    );
}

#[test]
fn builds_graph_ready_chunks_from_an_image_description() {
    let bytes = png_bytes(2, 1);
    let assembly = build_image_assembly(
        bytes.clone(),
        None,
        "evidence.png".to_string(),
        "analyst@example.com",
        "  A factory floor with three assembly lines.  ",
    )
    .unwrap();

    assert_eq!(assembly.document.file_name, "evidence.png");
    assert_eq!(assembly.document.source_type, "png");
    assert_eq!(assembly.document.file_size_bytes, bytes.len() as u64);
    assert_eq!(assembly.chunks.len(), 1);
    assert_eq!(
        assembly.chunks[0].text,
        "A factory floor with three assembly lines."
    );
    assert_eq!(
        assembly.chunks[0].section_title.as_deref(),
        Some("Image description")
    );
    assert!(assembly.chunks[0].embedding.is_none());
}

#[test]
fn decodes_supported_images_and_composites_transparency_on_white() {
    let decoded = decode_image(&png_bytes(2, 1), "image/png").unwrap();

    assert_eq!((decoded.width, decoded.height), (2, 1));
    assert_eq!(decoded.rgb_bytes.len(), 6);
    assert_eq!(&decoded.rgb_bytes[3..], &[255, 255, 255]);
}

#[test]
fn rejects_invalid_image_bytes_before_calling_openai() {
    assert!(decode_image(b"not a PNG", "image/png").is_err());
}

#[test]
fn rejects_images_above_the_pixel_limit() {
    assert!(supported_pixel_count(10_000, 10_000).is_err());
    assert_eq!(supported_pixel_count(4_000, 4_000).unwrap(), 16_000_000);
}

fn png_bytes(width: u32, height: u32) -> Vec<u8> {
    let image = RgbaImage::from_fn(width, height, |x, _| {
        if x == 0 {
            image::Rgba([12, 34, 56, 255])
        } else {
            image::Rgba([0, 0, 0, 0])
        }
    });
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut bytes, ImageOutputFormat::Png)
        .unwrap();
    bytes.into_inner()
}
