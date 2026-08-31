use std::{path::Path, path::PathBuf, sync::Arc};

use crate::core::data_room_helpers::{convert_office_bytes_to_pdf, convert_office_to_pdf};

#[derive(Clone, Debug)]
pub struct OfficeConverter {
    executable: Option<Arc<PathBuf>>,
}

impl OfficeConverter {
    pub fn new(executable: Option<PathBuf>) -> Self {
        Self {
            executable: executable.map(Arc::new),
        }
    }

    pub fn convert_path(&self, path: &Path) -> Result<Vec<u8>, String> {
        let executable = self.executable()?;
        convert_office_to_pdf(executable, path)
    }

    pub fn convert_bytes(&self, extension: &str, bytes: &[u8]) -> Result<Vec<u8>, String> {
        let executable = self.executable()?;
        convert_office_bytes_to_pdf(executable, extension, bytes)
    }

    fn executable(&self) -> Result<&Path, String> {
        self.executable.as_deref().map(PathBuf::as_path).ok_or_else(|| {
            "Office preview conversion is unavailable because LibreOffice/soffice was not configured. Set QUARRY_SOFFICE to its executable path."
                .to_string()
        })
    }
}
