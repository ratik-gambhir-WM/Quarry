use super::*;

#[test]
fn infers_supported_mime_types() {
    assert_eq!(
        infer_image_mime_type(Path::new("screenshot.PNG")).unwrap(),
        "image/png"
    );
    assert_eq!(
        infer_image_mime_type(Path::new("photo.jpeg")).unwrap(),
        "image/jpeg"
    );
    assert_eq!(
        infer_image_mime_type(Path::new("graphic.webp")).unwrap(),
        "image/webp"
    );
}
