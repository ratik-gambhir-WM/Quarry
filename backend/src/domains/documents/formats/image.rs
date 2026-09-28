use std::{
    io::Cursor,
    path::{Path, PathBuf},
};

use crate::{
    adapters::openai::client::{OpenAiClient, ResponsesFileInput},
    domains::documents::{
        formats::{image_prompt::IMAGE_DESCRIPTION_PROMPT, text_chunking::token_bounded_ranges},
        model::{Document, DocumentChunk},
    },
    shared::{file_policy::infer_supported_image_mime_type, ids::document_id_from_content},
};
use base64::{engine::general_purpose, Engine as _};
use image::{
    codecs::gif::GifDecoder,
    io::{Limits, Reader as ImageReader},
    AnimationDecoder, DynamicImage, ImageDecoder, ImageFormat,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const DEFAULT_IMAGE_DESCRIPTION_MODEL: &str = "gpt-5.5";
const MAX_IMAGE_DIMENSION: u32 = 20_000;
const MAX_IMAGE_DECODE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_IMAGE_PIXELS: u64 = 16_000_000;

#[derive(Debug, Clone)]
pub struct ImageAssembly {
    pub document: Document,
    pub chunks: Vec<DocumentChunk>,
}

pub(crate) struct DecodedImage {
    pub height: u32,
    pub rgb_bytes: Vec<u8>,
    pub width: u32,
}

pub(crate) struct ValidatedImage {
    pub bytes: Vec<u8>,
    pub mime_type: &'static str,
}

/// Validates bytes on the blocking pool and retains the MIME type derived from the filename.
/// Model calls and ingestion orchestration belong to the ingestion service.
pub(crate) async fn validate_image_by_bytes(
    bytes: Vec<u8>,
    file_name: &str,
) -> Result<ValidatedImage, String> {
    let mime_type = infer_supported_image_mime_type(Path::new(&file_name))
        .ok_or_else(|| format!("unsupported image file `{file_name}`"))?;
    let validation_mime_type = mime_type;
    let bytes = tokio::task::spawn_blocking(move || {
        validate_image(&bytes, validation_mime_type)?;
        Ok::<_, String>(bytes)
    })
    .await
    .map_err(|error| format!("image validation worker failed: {error}"))??;

    Ok(ValidatedImage { bytes, mime_type })
}

/// Legacy PDF image extraction still uses this default-model helper. Upload ingestion owns its
/// configured model selection and invokes OpenAI from its service instead.
pub async fn describe_image(
    image: &[u8],
    mime_type: &str,
    openai_client: &OpenAiClient,
) -> Result<String, String> {
    if image.is_empty() {
        return Err("cannot describe an empty image".to_string());
    }

    let normalized_mime_type = normalize_image_mime_type(mime_type)?;
    let image_base64 = general_purpose::STANDARD.encode(image);
    let file_inputs = [ResponsesFileInput::ImageData {
        mime_type: normalized_mime_type,
        data_base64: image_base64.as_str(),
        detail: Some("auto"),
    }];
    let description = openai_client
        .gen_model_response_with_files(
            Some(IMAGE_DESCRIPTION_PROMPT),
            None,
            Some(DEFAULT_IMAGE_DESCRIPTION_MODEL),
            Some(&file_inputs),
        )
        .await?;
    let description = description.trim().to_string();

    if description.is_empty() {
        return Err("OpenAI image analysis returned an empty description".to_string());
    }

    Ok(description)
}

pub(crate) fn build_image_assembly(
    bytes: Vec<u8>,
    path: Option<&Path>,
    file_name: String,
    user_id: &str,
    description: &str,
) -> Result<ImageAssembly, String> {
    let description = description.trim();
    if description.is_empty() {
        return Err("image description cannot be empty".to_string());
    }
    let file_size_bytes = u64::try_from(bytes.len())
        .map_err(|_| format!("image byte length `{}` does not fit in u64", bytes.len()))?;
    let content_hash = sha256_hex(&bytes);
    let document_id = document_id_from_content(user_id, &content_hash);
    let source_type = Path::new(&file_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| format!("could not infer image type for `{file_name}`"))?;
    let chunks = image_description_chunks(description, &document_id, user_id);
    let token_count = chunks
        .iter()
        .map(|chunk| u64::from(chunk.token_count))
        .sum();
    let local_path = path
        .map(|path| path.canonicalize().unwrap_or_else(|_| PathBuf::from(path)))
        .map(|path| path.to_string_lossy().into_owned());
    let document = Document {
        file_id: Uuid::new_v4().to_string(),
        document_id,
        user_id: user_id.to_string(),
        file_name,
        source_type,
        local_path,
        file_size_bytes,
        token_count,
        content_hash,
        rendered_pdf_path: None,
    };

    Ok(ImageAssembly { document, chunks })
}

fn image_description_chunks(
    description: &str,
    document_id: &str,
    user_id: &str,
) -> Vec<DocumentChunk> {
    token_bounded_ranges(description)
        .into_iter()
        .enumerate()
        .map(|(sequence_index, range)| {
            let sequence_number = u32::try_from(sequence_index + 1)
                .expect("image description chunk count should fit in u32");
            let text = &description[range.start_offset..range.end_offset];
            let content_hash = sha256_hex(text.as_bytes());
            let chunk_id = sha256_hex(
                format!("{user_id}\0{document_id}\0{sequence_number}\0{content_hash}").as_bytes(),
            );
            DocumentChunk {
                chunk_id,
                document_id: document_id.to_string(),
                user_id: user_id.to_string(),
                text: text.to_string(),
                embedding: None,
                sequence_number,
                page_numbers: None,
                start_offset: range.start_offset,
                end_offset: range.end_offset,
                token_count: u32::try_from(range.token_count)
                    .expect("image description chunk token count should fit in u32"),
                content_hash,
                section_title: Some("Image description".to_string()),
            }
        })
        .collect()
}

pub(crate) fn decode_image(bytes: &[u8], mime_type: &str) -> Result<DecodedImage, String> {
    let image = decode_dynamic_image(bytes, mime_type)?;
    let width = image.width();
    let height = image.height();
    let pixel_count = usize::try_from(supported_pixel_count(width, height)?)
        .map_err(|_| "image pixel count does not fit in memory".to_string())?;
    let rgba = image.to_rgba8();
    let rgb_capacity = pixel_count
        .checked_mul(3)
        .ok_or_else(|| "image RGB byte length overflowed".to_string())?;
    let mut rgb_bytes = Vec::with_capacity(rgb_capacity);
    for pixel in rgba.pixels() {
        let alpha = u16::from(pixel[3]);
        for channel in &pixel.0[..3] {
            let composited = (u16::from(*channel) * alpha + 255 * (255 - alpha)) / 255;
            rgb_bytes.push(u8::try_from(composited).expect("composited RGB channel fits in u8"));
        }
    }
    Ok(DecodedImage {
        height,
        rgb_bytes,
        width,
    })
}

fn validate_image(bytes: &[u8], mime_type: &str) -> Result<(), String> {
    decode_dynamic_image(bytes, mime_type).map(drop)
}

fn decode_dynamic_image(bytes: &[u8], mime_type: &str) -> Result<DynamicImage, String> {
    let normalized_mime_type = normalize_image_mime_type(mime_type)?;
    let format = image_format_for_mime_type(normalized_mime_type);
    let image = if format == ImageFormat::Gif {
        decode_single_frame_gif(bytes)?
    } else {
        let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
        reader.limits(image_decode_limits());
        reader
            .decode()
            .map_err(|error| format!("failed to decode {normalized_mime_type} image: {error}"))?
    };
    let width = image.width();
    let height = image.height();
    if width == 0 || height == 0 {
        return Err("image dimensions cannot be zero".to_string());
    }
    supported_pixel_count(width, height)?;
    Ok(image)
}

fn supported_pixel_count(width: u32, height: u32) -> Result<u64, String> {
    let pixel_count = u64::from(width) * u64::from(height);
    if pixel_count > MAX_IMAGE_PIXELS {
        return Err(format!(
            "image contains {pixel_count} pixels; limit is {MAX_IMAGE_PIXELS}"
        ));
    }
    Ok(pixel_count)
}

fn decode_single_frame_gif(bytes: &[u8]) -> Result<DynamicImage, String> {
    let mut decoder = GifDecoder::new(Cursor::new(bytes))
        .map_err(|error| format!("failed to decode image/gif image: {error}"))?;
    decoder
        .set_limits(image_decode_limits())
        .map_err(|error| format!("GIF image exceeds decoding limits: {error}"))?;
    let mut frames = decoder.into_frames();
    let first = frames
        .next()
        .transpose()
        .map_err(|error| format!("failed to decode image/gif frame: {error}"))?
        .ok_or_else(|| "GIF image does not contain a frame".to_string())?;
    if frames.next().is_some() {
        return Err("animated GIF images are not supported".to_string());
    }
    Ok(DynamicImage::ImageRgba8(first.into_buffer()))
}

fn image_decode_limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_IMAGE_DECODE_BYTES);
    limits
}

fn image_format_for_mime_type(mime_type: &str) -> ImageFormat {
    match mime_type {
        "image/png" => ImageFormat::Png,
        "image/jpeg" => ImageFormat::Jpeg,
        "image/webp" => ImageFormat::WebP,
        "image/gif" => ImageFormat::Gif,
        _ => unreachable!("normalized image MIME types are exhaustive"),
    }
}

fn normalize_image_mime_type(mime_type: &str) -> Result<&'static str, String> {
    match mime_type.trim().to_ascii_lowercase().as_str() {
        "image/png" => Ok("image/png"),
        "image/jpeg" | "image/jpg" => Ok("image/jpeg"),
        "image/webp" => Ok("image/webp"),
        "image/gif" => Ok("image/gif"),
        value => Err(format!(
            "unsupported image MIME type {value}; expected image/png, image/jpeg, image/webp, or image/gif"
        )),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/documents/formats/image_tests.rs"]
mod tests;
