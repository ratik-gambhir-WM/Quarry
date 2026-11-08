//! Pure relative-path parsing shared by request validation and canonical filesystem guards.
use std::path::{Component, Path};

pub(super) fn relative_document_path(relative_path: &str) -> Result<&Path, String> {
    let relative = Path::new(relative_path);
    if relative_path.trim().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(
            "document path must be a non-empty path relative to the deal data room".to_string(),
        );
    }
    Ok(relative)
}
