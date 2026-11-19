//! PowerPoint export and deck insertion.
use super::super::{
    normalize, pptx, Element, ExportedPowerPoint, FailureKind, InsertPowerPointInput, SlideResult,
    SlideTemplateError, TemplateAssetRepository,
};
use super::assets::{hydrate_template_assets, validate_image_sources};
use super::blocking;
use serde_json::Value;
use std::sync::Arc;

const MAX_JSON_DEPTH: usize = 100;
const MAX_JSON_NODES: usize = 100_000;

/// Converts presentation JSON into downloadable `.pptx` bytes.
#[derive(Clone)]
pub struct ExportService {
    repository: Arc<dyn TemplateAssetRepository>,
}

impl ExportService {
    #[must_use]
    pub fn new(repository: Arc<dyn TemplateAssetRepository>) -> Self {
        Self { repository }
    }

    /// Normalizes, validates, hydrates, and renders a presentation.
    ///
    /// # Errors
    ///
    /// Returns a stable API error for complex/invalid JSON, unsafe images, missing assets,
    /// oversized embedded images, or rendering failures.
    pub async fn export(&self, input: Value) -> SlideResult<ExportedPowerPoint> {
        let (mut presentation, warnings) = blocking(move || {
            assert_bounded_json(&input)?;
            let (mut presentation, warnings) = normalize::normalize(&input)?;
            // The reference's rendering normalizer drops empty image sources after canvas
            // normalization, without adding to the export's canvas-normalization warnings.
            for slide in &mut presentation.presentation.slides {
                slide.elements.retain(
                    |element| !matches!(element, Element::Image(image) if image.src.is_empty()),
                );
            }
            Ok((presentation, warnings))
        })
        .await?;
        hydrate_template_assets(&mut presentation, self.repository.as_ref()).await?;
        let (bytes, file_name) = blocking(move || {
            validate_image_sources(&presentation)?;
            let file_name = suggested_file_name(&presentation.presentation.title);
            let bytes = pptx::write(&presentation).map_err(SlideTemplateError::internal)?;
            Ok((bytes, file_name))
        })
        .await?;
        Ok(ExportedPowerPoint {
            bytes,
            file_name,
            warnings,
        })
    }

    /// Renders the supplied presentation and inserts it into an existing `.pptx` package.
    ///
    /// # Errors
    ///
    /// Returns a stable API error when the target deck or insert position is invalid, or when
    /// rendering/copying fails.
    pub async fn insert(&self, input: InsertPowerPointInput) -> SlideResult<ExportedPowerPoint> {
        if input.insert_after_slide > 9_007_199_254_740_991 {
            return Err(SlideTemplateError::new(
                FailureKind::Invalid,
                "invalid_insert_position",
                "insertAfterSlide must be a non-negative whole number.",
            ));
        }
        let target = blocking(move || {
            pptx::validate_package(&input.target_bytes)?;
            Ok(input.target_bytes)
        })
        .await?;
        let generated = self.export(input.presentation).await?;
        let generated_bytes = generated.bytes;
        let bytes = blocking(move || {
            pptx::insert(&generated_bytes, &target, input.insert_after_slide).map_err(|error| {
                SlideTemplateError::new(
                    FailureKind::Invalid,
                    "invalid_target_powerpoint",
                    "The target file is not a supported PowerPoint presentation.",
                )
                .with_source(error)
            })
        })
        .await?;
        Ok(ExportedPowerPoint {
            bytes,
            file_name: inserted_file_name(&input.target_file_name),
            warnings: generated.warnings,
        })
    }
}

fn inserted_file_name(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    for character in value.chars() {
        let character = if character.is_ascii_alphanumeric()
            || matches!(character, '.' | '_' | ' ' | '(' | ')' | '-')
        {
            character
        } else {
            '-'
        };
        if character != '-' || !sanitized.ends_with('-') {
            sanitized.push(character);
        }
    }
    let trimmed = sanitized.trim();
    let stem = if trimmed.to_ascii_lowercase().ends_with(".pptx") {
        &trimmed[..trimmed.len().saturating_sub(5)]
    } else {
        trimmed
    };
    format!(
        "{}-with-slide.pptx",
        if stem.is_empty() {
            "presentation"
        } else {
            stem
        }
    )
}

fn assert_bounded_json(input: &Value) -> SlideResult<()> {
    let mut pending = vec![(0_usize, input)];
    let mut node_count = 0_usize;
    while let Some((depth, value)) = pending.pop() {
        node_count = node_count.saturating_add(1);
        if node_count > MAX_JSON_NODES || depth > MAX_JSON_DEPTH {
            return Err(SlideTemplateError::new(
                FailureKind::Invalid,
                "presentation_too_complex",
                "The presentation JSON exceeds the processing complexity limit.",
            ));
        }
        match value {
            Value::Array(values) => pending.extend(values.iter().map(|value| (depth + 1, value))),
            Value::Object(values) => {
                pending.extend(values.values().map(|value| (depth + 1, value)));
            }
            Value::Number(number) if !number.as_f64().is_some_and(f64::is_finite) => {
                return Err(SlideTemplateError::new(
                    FailureKind::Invalid,
                    "invalid_json_value",
                    "The request body contains an unsupported JSON value.",
                ));
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
    Ok(())
}

fn suggested_file_name(title: &str) -> String {
    let mut slug = String::with_capacity(title.len());
    let mut needs_separator = false;
    for character in title.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            if needs_separator && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(character);
            needs_separator = false;
        } else {
            needs_separator = true;
        }
    }
    if slug.is_empty() {
        slug.push_str("generated-presentation");
    }
    slug.truncate(180);
    slug.push_str(".pptx");
    slug
}
