pub(crate) mod data_room;
pub(crate) mod deal;
pub(crate) mod documents;
pub(crate) mod research;
pub(crate) mod system;
pub(crate) mod users;

pub(crate) use crate::{errors::AppError, state::AppState};

pub(crate) async fn run_blocking<T, F>(task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tokio::task::spawn_blocking(task)
        .await
        .map_err(|error| format!("blocking worker failed: {error}"))?
}
