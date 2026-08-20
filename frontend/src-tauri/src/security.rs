use tauri::WebviewWindow;

use crate::errors::{AppError, AppResult};

pub(crate) fn verify_main_window_origin(window: &WebviewWindow) -> AppResult<()> {
    if window.label() != "main" {
        return Err(AppError::permission(
            "This window cannot access native file operations.",
        ));
    }

    let url = window.url().map_err(AppError::internal)?;
    let production_origin = (url.scheme() == "tauri" && url.host_str() == Some("localhost"))
        || (url.scheme() == "http" && url.host_str() == Some("tauri.localhost"));
    #[cfg(debug_assertions)]
    let development_origin = url.scheme() == "http"
        && matches!(url.host_str(), Some("localhost" | "127.0.0.1"))
        && url.port() == Some(1420);
    #[cfg(not(debug_assertions))]
    let development_origin = false;

    if production_origin || development_origin {
        Ok(())
    } else {
        Err(AppError::permission(
            "This origin cannot access native file operations.",
        ))
    }
}
