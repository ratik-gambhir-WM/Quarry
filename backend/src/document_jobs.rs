use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentJobEvent {
    pub job_id: String,
    pub filename: String,
    pub status: DocumentJobStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DocumentJobStatus {
    Processing,
    Completed,
    Skipped,
    Failed,
}

impl DocumentJobEvent {
    pub fn processing(job_id: String, filename: String) -> Self {
        Self {
            job_id,
            filename,
            status: DocumentJobStatus::Processing,
            document_id: None,
            chunk_count: None,
            error: None,
        }
    }

    pub fn completed(
        job_id: String,
        filename: String,
        document_id: Option<String>,
        chunk_count: usize,
    ) -> Self {
        Self {
            job_id,
            filename,
            status: DocumentJobStatus::Completed,
            document_id,
            chunk_count: Some(chunk_count),
            error: None,
        }
    }

    pub fn skipped(job_id: String, filename: String, document_id: Option<String>) -> Self {
        Self {
            job_id,
            filename,
            status: DocumentJobStatus::Skipped,
            document_id,
            chunk_count: None,
            error: None,
        }
    }

    pub fn failed(job_id: String, filename: String, error: String) -> Self {
        Self {
            job_id,
            filename,
            status: DocumentJobStatus::Failed,
            document_id: None,
            chunk_count: None,
            error: Some(error),
        }
    }

    pub fn event_name(&self) -> &'static str {
        match self.status {
            DocumentJobStatus::Processing => "processing",
            DocumentJobStatus::Completed => "completed",
            DocumentJobStatus::Skipped => "skipped",
            DocumentJobStatus::Failed => "failed",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            DocumentJobStatus::Completed | DocumentJobStatus::Skipped | DocumentJobStatus::Failed
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skipped_jobs_are_terminal_skipped_events() {
        let event = DocumentJobEvent::skipped(
            "job-1".to_string(),
            "memo.pdf".to_string(),
            Some("document-1".to_string()),
        );

        assert_eq!(event.event_name(), "skipped");
        assert!(event.is_terminal());
        assert_eq!(event.document_id.as_deref(), Some("document-1"));
        assert!(event.chunk_count.is_none());
    }
}
