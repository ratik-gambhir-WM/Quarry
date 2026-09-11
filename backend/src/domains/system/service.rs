use std::{path::PathBuf, sync::Arc};

#[derive(Clone)]
pub struct DatabaseService {
    path: Arc<PathBuf>,
}

impl DatabaseService {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path: Arc::new(path),
        }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}
