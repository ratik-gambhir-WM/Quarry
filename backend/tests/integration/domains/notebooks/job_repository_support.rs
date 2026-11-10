use super::*;

impl NotebookRepository {
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
