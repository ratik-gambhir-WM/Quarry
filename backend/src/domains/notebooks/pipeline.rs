//! Bounded provider preparation. SQL publication belongs to the notebook repository.
use super::model::PIPELINE;
use crate::adapters::{openai::client::OpenAiClient, postgres::vector::CosineVector};
use futures_util::future::BoxFuture;
use std::{num::NonZeroUsize, sync::Arc};

pub(super) trait SavePipeline: Send + Sync {
    fn fingerprint(&self) -> String;
    fn dimensions(&self) -> usize;
    fn embed<'a>(
        &'a self,
        chunks: &'a [String],
    ) -> BoxFuture<'a, Result<Vec<CosineVector>, &'static str>>;
}
pub(crate) struct NotebookPipeline {
    openai: Option<Arc<OpenAiClient>>,
    dimensions: NonZeroUsize,
    model: String,
}
impl NotebookPipeline {
    pub(crate) fn new(
        openai: Option<Arc<OpenAiClient>>,
        dimensions: NonZeroUsize,
        model: String,
    ) -> Self {
        Self {
            openai,
            dimensions,
            model,
        }
    }
}
impl SavePipeline for NotebookPipeline {
    fn fingerprint(&self) -> String {
        format!("{PIPELINE}/openai/{}/{}", self.model, self.dimensions)
    }
    fn dimensions(&self) -> usize {
        self.dimensions.get()
    }
    fn embed<'a>(
        &'a self,
        chunks: &'a [String],
    ) -> BoxFuture<'a, Result<Vec<CosineVector>, &'static str>> {
        Box::pin(async move {
            if chunks.is_empty() {
                return Ok(Vec::new());
            }
            let openai = self.openai.as_ref().ok_or("embedding_unavailable")?;
            let mut vectors = Vec::with_capacity(chunks.len());
            for batch in chunks.chunks(16) {
                let inputs = batch.iter().map(String::as_str).collect::<Vec<_>>();
                let result = openai
                    .gen_private_embeddings(&inputs, Some(&self.model))
                    .await
                    .map_err(|_| "embedding_failed")?;
                if result.len() != batch.len() {
                    return Err("embedding_failed");
                }
                for vector in result {
                    if vector.len() != self.dimensions.get() {
                        return Err("embedding_failed");
                    }
                    let vector = CosineVector::from_f64(&vector, self.dimensions.get())
                        .map_err(|_| "embedding_failed")?;
                    vectors.push(vector);
                }
            }
            Ok(vectors)
        })
    }
}
