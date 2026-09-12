use reqwest::Url;

pub(super) const DILIGENCE_STUDIO_APP_ID: &str = "Quarry_WestMonroe";
pub(super) const APP_ID_HEADER: &str = "X-App-Id";

#[derive(Clone)]
pub struct DiligenceStudioClient {
    pub(super) http: reqwest::Client,
    pub(super) base_url: Url,
}

impl DiligenceStudioClient {
    pub fn new(http: reqwest::Client, base_url: Url) -> Self {
        Self { http, base_url }
    }

    pub(super) fn endpoint(&self, relative_path: &str) -> Result<Url, String> {
        self.base_url
            .join(relative_path)
            .map_err(|error| format!("failed to build Diligence Studio endpoint: {error}"))
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/adapters/diligence_studio/client_tests.rs"]
mod tests;
