use super::*;

impl NotebookRepository {
    pub(in crate::domains::notebooks) async fn commit_test(
        &self,
        claim: &ClaimedJob,
    ) -> Result<()> {
        let markdown = super::super::markdown::export(&claim.input.note).unwrap();
        let chunks = super::super::markdown::chunks(&markdown).unwrap();
        let vectors = chunks
            .iter()
            .map(|_| {
                crate::adapters::postgres::vector::CosineVector::new(vec![0.5; 1536], 1536).unwrap()
            })
            .collect();
        let prepared = PreparedNote::new(
            &claim.input.note,
            claim.input.pipeline_version.clone(),
            1536,
            markdown,
            chunks,
            vectors,
        )?;
        self.persist(claim, Some(prepared), None).await
    }

    pub(in crate::domains::notebooks) async fn admit(
        &self,
        owner: i64,
        notebook: String,
        note_id: Option<String>,
        input: SaveInput,
    ) -> Result<SaveJob> {
        self.admit_with_pipeline(owner, notebook, note_id, input, PIPELINE.into())
            .await
    }
}
