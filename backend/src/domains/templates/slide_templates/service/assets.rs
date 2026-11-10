//! Template-owned image externalization and hydration for import and export.

use std::{borrow::Cow, collections::HashMap};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use uuid::Uuid;

use super::super::{
    CanvasDocument, Element, FailureKind, SlideResult, SlideTemplateError, TemplateAsset,
    TemplateRepository,
};
use super::document_too_large;

const MAX_EMBEDDED_IMAGE_BYTES: usize = 40 * 1024 * 1024;

pub(super) fn externalize_assets(
    template_id: &str,
    document: CanvasDocument,
) -> SlideResult<(CanvasDocument, Vec<TemplateAsset>)> {
    externalize_assets_with(template_id, document, || Uuid::new_v4().to_string())
}

fn externalize_assets_with(
    template_id: &str,
    mut document: CanvasDocument,
    mut create_id: impl FnMut() -> String,
) -> SlideResult<(CanvasDocument, Vec<TemplateAsset>)> {
    let mut assets = Vec::new();
    let mut source_to_id = HashMap::<String, String>::new();
    for slide in &mut document.presentation.slides {
        for element in &mut slide.elements {
            let Element::Image(image) = element else {
                continue;
            };
            if parse_asset_path(&image.src).is_some() {
                continue;
            }
            if let Some(asset_id) = source_to_id.get(&image.src) {
                image.src = asset_path(template_id, asset_id);
                continue;
            }
            let (content_type, bytes) = decode_data_image(&image.src).map_err(|error| {
                SlideTemplateError::new(
                    FailureKind::Invalid,
                    "unsupported_imported_image_source",
                    "An imported image could not be stored safely.",
                )
                .with_source(error)
            })?;
            if bytes.is_empty() {
                return Err(SlideTemplateError::new(
                    FailureKind::Invalid,
                    "invalid_imported_image",
                    "An imported image did not contain valid image data.",
                ));
            }
            let asset_id = create_id();
            source_to_id.insert(image.src.clone(), asset_id.clone());
            assets.push(TemplateAsset {
                asset_id: asset_id.clone(),
                template_id: template_id.to_owned(),
                content_type,
                bytes,
            });
            image.src = asset_path(template_id, &asset_id);
        }
    }
    Ok((document, assets))
}

pub(super) fn hydrate_template_assets(
    document: &mut CanvasDocument,
    repository: &dyn TemplateRepository,
) -> SlideResult<()> {
    hydrate_assets(document, |template_id, asset_id| {
        repository
            .find_asset(template_id, asset_id)
            .map(|asset| asset.map(Cow::Owned))
            .map_err(SlideTemplateError::internal)
    })
}

pub(super) fn hydrate_canvas_assets(
    mut document: CanvasDocument,
    assets: &[TemplateAsset],
) -> SlideResult<CanvasDocument> {
    let assets_by_reference = assets
        .iter()
        .map(|asset| ((asset.template_id.as_str(), asset.asset_id.as_str()), asset))
        .collect::<HashMap<_, _>>();
    hydrate_assets(&mut document, |template_id, asset_id| {
        Ok(assets_by_reference
            .get(&(template_id, asset_id))
            .map(|asset| Cow::Borrowed(*asset)))
    })?;
    Ok(document)
}

// Both stored and freshly imported assets use the same reference, byte-budget and encoding
// rules. Borrow in-memory assets while allowing repository reads to return owned values.
fn hydrate_assets<'a>(
    document: &mut CanvasDocument,
    mut find_asset: impl FnMut(&str, &str) -> SlideResult<Option<Cow<'a, TemplateAsset>>>,
) -> SlideResult<()> {
    let mut hydrated_bytes = 0usize;
    for slide in &mut document.presentation.slides {
        for element in &mut slide.elements {
            let Element::Image(image) = element else {
                continue;
            };
            let Some((template_id, asset_id)) = parse_asset_path(&image.src) else {
                continue;
            };
            let asset = find_asset(template_id, asset_id)?.ok_or_else(|| {
                SlideTemplateError::new(
                    FailureKind::Invalid,
                    "template_asset_not_found",
                    "A referenced template image does not exist.",
                )
            })?;
            hydrated_bytes = hydrated_bytes.saturating_add(asset.bytes.len());
            if hydrated_bytes > MAX_EMBEDDED_IMAGE_BYTES {
                return Err(document_too_large());
            }
            image.src = format!(
                "data:{};base64,{}",
                asset.content_type,
                STANDARD.encode(&asset.bytes)
            );
        }
    }
    Ok(())
}

pub(super) fn validate_image_sources(document: &CanvasDocument) -> SlideResult<()> {
    let mut embedded_bytes = 0_usize;
    for image in document
        .presentation
        .slides
        .iter()
        .flat_map(|slide| &slide.elements)
        .filter_map(|element| match element {
            Element::Image(image) => Some(image),
            Element::Text(_) | Element::Shape(_) | Element::Line(_) => None,
        })
    {
        let (_, bytes) = decode_data_image(&image.src).map_err(|_| {
            SlideTemplateError::new(
                FailureKind::Invalid,
                "unsupported_image_source",
                "Exported images must use an embedded base64 data URI.",
            )
        })?;
        embedded_bytes = embedded_bytes.saturating_add(bytes.len());
        if embedded_bytes > MAX_EMBEDDED_IMAGE_BYTES {
            return Err(SlideTemplateError::new(
                FailureKind::TooLarge,
                "embedded_images_too_large",
                "The embedded presentation images exceed the processing limit.",
            ));
        }
    }
    Ok(())
}

fn decode_data_image(source: &str) -> anyhow::Result<(String, Vec<u8>)> {
    let Some(rest) = source.strip_prefix("data:") else {
        anyhow::bail!("image is not a data URI");
    };
    let Some((content_type, encoded)) = rest.split_once(";base64,") else {
        anyhow::bail!("image data URI is not base64 encoded");
    };
    let content_type = content_type.to_ascii_lowercase();
    anyhow::ensure!(
        matches!(
            content_type.as_str(),
            "image/png" | "image/jpeg" | "image/jpg" | "image/gif" | "image/svg+xml" | "image/emf"
        ),
        "unsupported image content type"
    );
    anyhow::ensure!(
        encoded.len() <= MAX_EMBEDDED_IMAGE_BYTES.saturating_mul(4).div_ceil(3) + 4,
        "embedded image exceeds processing limit"
    );
    let bytes = STANDARD.decode(encoded)?;
    Ok((content_type, bytes))
}

fn parse_asset_path(source: &str) -> Option<(&str, &str)> {
    let mut segments = source
        .strip_prefix("/api/v1/templates/slide-templates/import/")
        .or_else(|| source.strip_prefix("/import/"))?
        .split('/');
    let template_id = segments.next()?;
    if segments.next()? != "assets" {
        return None;
    }
    let asset_id = segments.next()?;
    if segments.next().is_some() || !valid_id(template_id) || !valid_id(asset_id) {
        return None;
    }
    Some((template_id, asset_id))
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn asset_path(template_id: &str, asset_id: &str) -> String {
    format!("/api/v1/templates/slide-templates/import/{template_id}/assets/{asset_id}")
}

#[cfg(test)]
#[path = "../../../../../tests/unit/domains/templates/slide_templates/service/assets_tests.rs"]
mod tests;
