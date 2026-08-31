mod deal_extraction;
mod document_summary;
mod helix_query;
mod image_description;
mod responses;

pub use deal_extraction::{
    build_deal_extraction_prompt, DealExtractionPromptVariables, DEAL_EXTRACTION_SYSTEM_PROMPT,
};
pub use document_summary::{
    build_basic_document_summary_prompt, build_document_summary_prompt,
    CLI_DOCUMENT_SUMMARY_SYSTEM_PROMPT, DOCUMENT_SUMMARY_SYSTEM_PROMPT,
};
pub use helix_query::HELIX_QUERY_EXAMPLE_PROMPT;
pub use image_description::IMAGE_DESCRIPTION_PROMPT;
pub use responses::{DEFAULT_RESPONSES_PROMPT, DEFAULT_SYSTEM_INSTRUCTIONS};
