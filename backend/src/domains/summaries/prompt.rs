use std::path::Path;

use crate::shared::{
    file_policy::CollectedFile,
    prompts::{PromptCatalog, PromptId},
};

pub fn build_document_summary_prompt(
    prompts: &PromptCatalog,
    root: &Path,
    files: &[CollectedFile],
    skipped_files: &[String],
) -> String {
    format!(
        "{}\n\n{}\n\n{}",
        build_basic_document_summary_prompt(prompts, root, files, skipped_files),
        prompts.prompt(PromptId::TechnologyDiligence),
        prompts.prompt(PromptId::ProductApplicationDeepDive),
    )
}

pub fn build_basic_document_summary_prompt(
    prompts: &PromptCatalog,
    root: &Path,
    files: &[CollectedFile],
    skipped_files: &[String],
) -> String {
    let manifest = files
        .iter()
        .map(|file| {
            format!(
                "- {} ({}, {} bytes)",
                file.relative_path, file.mime_type, file.size_bytes
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let skipped = if skipped_files.is_empty() {
        "No files were skipped.".to_string()
    } else {
        format!(
            "The following files were skipped because they are unsupported or empty:\n{}",
            skipped_files
                .iter()
                .map(|path| format!("- {path}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };

    prompts.render_basic_summary(&root.display().to_string(), &manifest, &skipped)
}
