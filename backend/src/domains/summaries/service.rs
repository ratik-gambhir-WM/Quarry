use std::{path::Path, sync::Arc};

use crate::{
    adapters::openai::client::{OpenAiClient, ResponsesFileInput},
    domains::summaries::prompt::build_document_summary_prompt,
    shared::{
        error::{ServiceError, ServiceResult},
        file_policy::CollectedFile,
        prompts::{PromptCatalog, PromptId},
    },
};

pub struct SummaryService {
    openai: Option<Arc<OpenAiClient>>,
    model: String,
    prompts: Arc<PromptCatalog>,
}

impl SummaryService {
    pub fn new(
        openai: Option<Arc<OpenAiClient>>,
        model: String,
        prompts: Arc<PromptCatalog>,
    ) -> Self {
        Self {
            openai,
            model,
            prompts,
        }
    }

    pub async fn summarize_collected_files(
        &self,
        root_label: &str,
        files: Vec<CollectedFile>,
        skipped_files: Vec<String>,
    ) -> ServiceResult<String> {
        if files.is_empty() {
            return Err(ServiceError::validation(format!(
                "no supported files found in {root_label}"
            )));
        }
        let client = self
            .openai
            .as_ref()
            .ok_or_else(|| ServiceError::unavailable("OpenAI capability is not configured"))?;
        let root = Path::new(root_label);
        let prompt = build_document_summary_prompt(&self.prompts, root, &files, &skipped_files);
        let file_inputs: Vec<ResponsesFileInput<'_>> = files
            .iter()
            .map(|file| ResponsesFileInput::FileData {
                filename: file.filename.as_str(),
                mime_type: file.mime_type,
                data_base64: file.data_base64.as_str(),
            })
            .collect();

        client
            .gen_model_response_with_files(
                Some(&prompt),
                Some(self.prompts.prompt(PromptId::SummarySystem)),
                Some(&self.model),
                Some(&file_inputs),
            )
            .await
            .map_err(ServiceError::internal)
    }
}
