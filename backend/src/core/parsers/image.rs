#![allow(dead_code)]

use std::{fs, path::Path};

use crate::core::{
    clients::openai::{OpenAiClient, ResponsesFileInput},
    prompts::IMAGE_DESCRIPTION_PROMPT,
};
use base64::{engine::general_purpose, Engine as _};

const DEFAULT_IMAGE_DESCRIPTION_MODEL: &str = "gpt-5.5";

pub async fn parse_image_file(
    image_path: &Path,
    openai_client: &OpenAiClient,
) -> Result<String, String> {
    let image = fs::read(image_path)
        .map_err(|err| format!("failed to read image file {}: {err}", image_path.display()))?;
    let mime_type = infer_image_mime_type(image_path)?;

    describe_image(&image, mime_type, openai_client).await
}

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

fn infer_image_mime_type(image_path: &Path) -> Result<&'static str, String> {
    match image_path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => Ok("image/png"),
        Some("jpg") | Some("jpeg") => Ok("image/jpeg"),
        Some("webp") => Ok("image/webp"),
        Some("gif") => Ok("image/gif"),
        Some(extension) => Err(format!(
            "unsupported image extension .{extension}; expected png, jpg, jpeg, webp, or gif"
        )),
        None => Err(format!(
            "could not infer image type for {}; pass bytes with an explicit MIME type instead",
            image_path.display()
        )),
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

#[cfg(test)]
#[path = "../../../tests/core/parsers/image_tests.rs"]
mod tests;
