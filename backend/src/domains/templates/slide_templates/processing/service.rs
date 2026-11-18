//! Prepare provider results and publish them under the admitted stage claim.
use super::super::{
    providers::{SlideClassifier, SlideEmbedder, SlideProviderError},
    retrieval::*,
    TemplateProcessingRepository,
};
use std::sync::Arc;

pub struct SlideClassificationServiceOptions {
    pub classifier: Arc<dyn SlideClassifier>,
    pub classification_model: String,
    pub embedder: Arc<dyn SlideEmbedder>,
    pub embedding_dimensions: Option<usize>,
    pub embedding_max_text_bytes: usize,
    pub embedding_model: String,
    pub input_limits: SlideClassificationInputLimits,
    pub repository: Arc<dyn TemplateProcessingRepository>,
}

pub struct SlideClassificationService {
    pub options: SlideClassificationServiceOptions,
}
impl SlideClassificationService {
    pub fn new(options: SlideClassificationServiceOptions) -> Self {
        Self { options }
    }
    /// Classify outside SQL, then publish metadata/lexical input under the job's claim.
    pub async fn classify(&self, job: SlideClassificationJob) -> anyhow::Result<()> {
        let claim = job.claim.clone();
        let result = self.prepare_classification(job).await?;
        let repository = self.options.repository.clone();
        tokio::spawn(async move {
            repository
                .complete_slide_classification(&claim, &result)
                .await
        })
        .await?
        .map_err(Into::into)
    }
    /// Embed both roles outside SQL; one failed role publishes nothing.
    pub async fn embed(&self, job: &SlideEmbeddingJob) -> anyhow::Result<()> {
        let prepared = self.prepare_embeddings(job).await?;
        let repository = self.options.repository.clone();
        let claim = job.claim.clone();
        tokio::spawn(async move { repository.complete_slide_embedding(&claim, &prepared).await })
            .await?
            .map_err(Into::into)
    }
    /// Obtain and validate the complete pair before any mutation.
    pub async fn prepare_embeddings(
        &self,
        job: &SlideEmbeddingJob,
    ) -> anyhow::Result<PreparedSlideEmbeddings> {
        let (subject_vector, capability_vector) = tokio::try_join!(
            self.options.embedder.embed(&job.subject_document),
            self.options.embedder.embed(&job.capability_document),
        )?;
        let dimensions = self
            .options
            .embedding_dimensions
            .unwrap_or(subject_vector.len());
        validate_embedding(&subject_vector, Some(dimensions))?;
        validate_embedding(&capability_vector, Some(dimensions))?;
        PreparedSlideEmbeddings::new(
            TemplateEmbeddingProfile::new(self.options.embedding_model.clone(), dimensions),
            job.subject_fingerprint.clone(),
            &subject_vector,
            job.capability_fingerprint.clone(),
            &capability_vector,
        )
    }

    async fn prepare_classification(
        &self,
        job: SlideClassificationJob,
    ) -> anyhow::Result<CompleteSlideClassification> {
        let limits = self.options.input_limits.clone();
        let (job, input) = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
            let input = build_slide_classification_input(
                &job.template.template_json,
                job.kind,
                Some(&job.title),
                job.preview.as_ref(),
                &limits,
            )?;
            if job.preview.is_some() && input.preview.is_none() {
                return Err(
                    SlideProviderError::new("classification_preview_unavailable", false).into(),
                );
            }
            Ok((job, input))
        })
        .await??;
        let metadata = self.options.classifier.classify(&input).await?;
        let element_ids = job.template.template_json.presentation.slides[0]
            .elements
            .iter()
            .map(|element| element.id().to_owned())
            .collect();
        assert_content_slots_exist(&metadata, &element_ids)?;
        let documents = build_slide_embedding_documents(
            &job.title,
            job.kind,
            &metadata,
            self.options.embedding_max_text_bytes,
        )?;
        let subject_embedding_fingerprint = build_slide_embedding_fingerprint(
            "subject",
            &documents.subject,
            &self.options.embedding_model,
            self.options.embedding_dimensions,
        );
        let capability_embedding_fingerprint = build_slide_embedding_fingerprint(
            "capability",
            &documents.capability,
            &self.options.embedding_model,
            self.options.embedding_dimensions,
        );
        Ok(CompleteSlideClassification {
            classified_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            classified_fingerprint: job.input_fingerprint.clone(),
            subject_embedding_document: documents.subject,
            subject_embedding_fingerprint,
            capability_embedding_document: documents.capability,
            capability_embedding_fingerprint,
            metadata,
            model: self.options.classification_model.clone(),
            template_id: job.template.template_id.clone(),
        })
    }
}

#[cfg(test)]
#[path = "../../../../../tests/unit/domains/templates/slide_templates/processing/service_tests.rs"]
mod tests;
