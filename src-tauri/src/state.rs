use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use rusqlite::Connection;
use tauri::AppHandle;

use crate::core::clients::{helix::HelixClient, sqlite::SqliteClient};
use crate::document_jobs::DocumentJobManager;

#[derive(Clone)]
pub struct AppState {
    helix_client: Arc<HelixClient>,
    sqlite_client: Arc<SqliteClient>,
    document_jobs: Arc<DocumentJobManager>,
    path_grants: Arc<Mutex<HashSet<PathBuf>>>,
}

impl AppState {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        Ok(Self {
            helix_client: Arc::new(HelixClient::new()?),
            sqlite_client: Arc::new(SqliteClient::new(app)?),
            document_jobs: Arc::new(DocumentJobManager::default()),
            path_grants: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    #[cfg(test)]
    pub fn new_for_test() -> Result<Self, String> {
        Ok(Self {
            helix_client: Arc::new(HelixClient::new()?),
            sqlite_client: Arc::new(SqliteClient::new_in_memory()?),
            document_jobs: Arc::new(DocumentJobManager::default()),
            path_grants: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    pub fn gen_helix_db_client(&self) -> &HelixClient {
        self.helix_client.as_ref()
    }

    pub fn gen_sqlite_db_client(&self) -> &SqliteClient {
        self.sqlite_client.as_ref()
    }

    pub fn sqlite_db_path(&self) -> &Path {
        &self.sqlite_client.db_path
    }

    pub fn document_jobs(&self) -> &DocumentJobManager {
        self.document_jobs.as_ref()
    }

    pub fn grant_paths(&self, paths: impl IntoIterator<Item = PathBuf>) -> Result<(), String> {
        let new_paths = paths.into_iter().collect::<Vec<_>>();
        let mut grants = self
            .path_grants
            .lock()
            .map_err(|_| "local file grants are unavailable".to_string())?;
        if grants.len() + new_paths.len() > 500 {
            grants.clear();
        }
        grants.extend(new_paths);
        Ok(())
    }

    pub fn is_path_granted(&self, path: &Path) -> Result<bool, String> {
        self.path_grants
            .lock()
            .map(|grants| grants.contains(path))
            .map_err(|_| "local file grants are unavailable".to_string())
    }

    pub fn is_path_authorized(&self, path: &Path) -> Result<bool, String> {
        self.path_grants
            .lock()
            .map(|grants| {
                grants
                    .iter()
                    .any(|grant| path == grant || path.starts_with(grant))
            })
            .map_err(|_| "local file grants are unavailable".to_string())
    }

    pub fn with_sqlite_db<T>(
        &self,
        f: impl FnOnce(&Connection) -> rusqlite::Result<T>,
    ) -> Result<T, String> {
        self.sqlite_client.with_connection(f)
    }
}
