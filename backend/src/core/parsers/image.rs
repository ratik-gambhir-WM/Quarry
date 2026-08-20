#![allow(dead_code)]

use std::{env, fs, path::Path};

use crate::core::clients::openai::{OpenAiClient, ResponsesFileInput};
use base64::{engine::general_purpose, Engine as _};

const DEFAULT_IMAGE_DESCRIPTION_MODEL: &str = "gpt-5.4";

const IMAGE_DESCRIPTION_PROMPT: &str = r#"Create a dense, retrieval-optimized description of this image for semantic and keyword search. Explain what the image is, its context, its apparent purpose, and the main idea or information it is trying to convey. Prioritize meaningful domain concepts and relationships over a catalog of purely visual details. Use concrete terminology, visible names, and explicit relationships that someone might search for later.

Accurately capture relevant subjects, objects, people, settings, actions, screenshots, document structure, readable labels and text, diagrams, tables, charts, forms, product or brand clues, and meaningful visual relationships. For structured content, explain what it communicates, how it is organized, and how its labeled elements relate when those relationships are visible.

Clearly distinguish directly visible facts from cautious interpretations. Qualify any inference and never invent details, behavior, labels, values, or relationships that are not visible or reasonably supported. Avoid filler, markdown, and generic phrases such as "this image shows." Write as much as necessary to preserve faithful, useful context for retrieval."#;

pub async fn parse_image_file(
    image_path: &Path,
    openai_client: &OpenAiClient<'_>,
) -> Result<String, String> {
    let image = fs::read(image_path)
        .map_err(|err| format!("failed to read image file {}: {err}", image_path.display()))?;
    let mime_type = infer_image_mime_type(image_path)?;

    describe_image(&image, mime_type, openai_client).await
}

pub async fn describe_image(
    image: &[u8],
    mime_type: &str,
    openai_client: &OpenAiClient<'_>,
) -> Result<String, String> {
    if image.is_empty() {
        return Err("cannot describe an empty image".to_string());
    }

    let normalized_mime_type = normalize_image_mime_type(mime_type)?;
    let model = env::var("OPENAI_IMAGE_DESCRIPTION_MODEL")
        .unwrap_or_else(|_| DEFAULT_IMAGE_DESCRIPTION_MODEL.to_string());
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
            Some(&model),
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
