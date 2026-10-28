use std::{
    collections::HashMap,
    env, fmt,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    num::NonZeroUsize,
    path::PathBuf,
    time::Duration,
};

use axum::http::HeaderValue;

const DEFAULT_API_PORT: u16 = 3001;
const DEFAULT_REQUEST_TIMEOUT_SECONDS: u64 = 120;
const DEFAULT_CORS_ORIGINS: &str = "http://127.0.0.1:1420,http://localhost:1420";
const DEFAULT_DATABASE_FILE_NAME: &str = "quarry.sqlite3";
const DEFAULT_HELIX_URL: &str = "http://127.0.0.1:6969";
const DEFAULT_HELIX_VECTOR_DIMENSION: usize = 1536;
const DEFAULT_DEAL_EXTRACTION_MODEL: &str = "gpt-5.6-luna";
const DEFAULT_EMBEDDING_MODEL: &str = "text-embedding-3-small";
const DEFAULT_DOCUMENT_SUMMARY_MODEL: &str = "gpt-5.5";
const DEFAULT_IMAGE_DESCRIPTION_MODEL: &str = "gpt-5.5";
const DEFAULT_CHAT_MODEL: &str = "gpt-5.5";
const DEFAULT_DOCUMENT_CONCURRENCY: usize = 8;
const DEFAULT_COMPLETED_JOB_RETENTION_SECONDS: u64 = 10 * 60;

#[derive(Clone)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

#[derive(Clone, Debug)]
pub struct HttpConfig {
    pub bind_address: SocketAddr,
    pub cors_origins: Vec<HeaderValue>,
    pub request_timeout: Duration,
}

#[derive(Clone, Debug)]
pub struct SqliteConfig {
    pub path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct HelixConfig {
    pub url: String,
    pub api_key: Option<SecretString>,
    pub vector_dimension: NonZeroUsize,
}

#[derive(Clone, Debug)]
pub struct OpenAiConfig {
    pub api_key: SecretString,
    pub chat_model: String,
    pub deal_extraction_model: String,
    pub embedding_model: String,
    pub embedding_dimensions: NonZeroUsize,
    pub document_summary_model: String,
    pub image_description_model: String,
}

#[derive(Clone, Debug)]
pub struct WmAiConfig {
    pub file_upload_url: String,
    pub file_upload_api_key: SecretString,
    pub index_url: String,
    pub index_api_key: SecretString,
    pub graph_rag_url: String,
    pub graph_rag_api_key: SecretString,
    pub graph_rag_application_name: String,
}

#[derive(Clone, Debug, Default)]
pub struct DataRoomConfig {
    pub roots: HashMap<String, PathBuf>,
    pub office_executable: Option<PathBuf>,
}

impl DataRoomConfig {
    pub fn root_for_deal(&self, deal_id: &str) -> Option<&PathBuf> {
        self.roots.get(&normalize_deal_config_key(deal_id))
    }
}

#[derive(Clone, Debug)]
pub struct DocumentConfig {
    pub max_concurrent_documents: usize,
    pub completed_job_retention: Duration,
}

#[derive(Clone, Debug)]
pub struct DiligenceStudioConfig {
    pub base_url: reqwest::Url,
}

#[derive(Clone, Debug)]
pub struct PromptConfig {
    pub directory: PathBuf,
}

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub http: HttpConfig,
    pub sqlite: SqliteConfig,
    pub helix: HelixConfig,
    pub openai: Option<OpenAiConfig>,
    pub wm_ai: Option<WmAiConfig>,
    pub data_room: DataRoomConfig,
    pub documents: DocumentConfig,
    pub diligence_studio: Option<DiligenceStudioConfig>,
    pub slide_templates: SlideTemplatesConfig,
    pub prompts: PromptConfig,
}

impl AppConfig {
    /// The only production adapter from ambient process configuration.
    pub fn from_env() -> Result<Self, String> {
        Self::from_values(env::vars())
    }

    /// Parses configuration from an injected key/value source. This avoids
    /// mutating process-global environment variables in parallel tests.
    pub fn from_values<I, K, V>(values: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let values = values
            .into_iter()
            .map(|(key, value)| (key.into(), value.into()))
            .collect::<HashMap<_, _>>();

        let helix = parse_helix_config(&values)?;
        let openai = parse_openai_config(&values, helix.vector_dimension)?;

        Ok(Self {
            http: parse_http_config(&values)?,
            sqlite: parse_sqlite_config(&values),
            helix,
            openai,
            wm_ai: parse_wm_ai_config(&values)?,
            data_room: parse_data_room_config(&values),
            documents: parse_document_config(&values)?,
            diligence_studio: parse_diligence_studio_config(&values)?,
            slide_templates: parse_slide_templates_config(&values)?,
            prompts: parse_prompt_config(&values)?,
        })
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            http: HttpConfig {
                bind_address: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), DEFAULT_API_PORT),
                cors_origins: DEFAULT_CORS_ORIGINS
                    .split(',')
                    .map(|origin| {
                        HeaderValue::from_str(origin).expect("default CORS origin is valid")
                    })
                    .collect(),
                request_timeout: Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECONDS),
            },
            sqlite: SqliteConfig {
                path: PathBuf::from("data").join(DEFAULT_DATABASE_FILE_NAME),
            },
            helix: HelixConfig {
                url: DEFAULT_HELIX_URL.to_string(),
                api_key: None,
                vector_dimension: NonZeroUsize::new(DEFAULT_HELIX_VECTOR_DIMENSION)
                    .expect("default Helix vector dimension is non-zero"),
            },
            openai: None,
            wm_ai: None,
            data_room: DataRoomConfig::default(),
            documents: DocumentConfig {
                max_concurrent_documents: DEFAULT_DOCUMENT_CONCURRENCY,
                completed_job_retention: Duration::from_secs(
                    DEFAULT_COMPLETED_JOB_RETENTION_SECONDS,
                ),
            },
            diligence_studio: None,
            slide_templates: parse_slide_templates_config(&HashMap::new())
                .expect("default slide configuration is valid"),
            prompts: PromptConfig {
                directory: PathBuf::from("prompts"),
            },
        }
    }
}

fn parse_prompt_config(values: &HashMap<String, String>) -> Result<PromptConfig, String> {
    let directory = match values.get("QUARRY_PROMPTS_DIR") {
        Some(value) if value.trim().is_empty() => {
            return Err("QUARRY_PROMPTS_DIR must not be blank".to_string());
        }
        Some(value) => PathBuf::from(value.trim()),
        None => PathBuf::from("prompts"),
    };
    Ok(PromptConfig { directory })
}

fn parse_diligence_studio_config(
    values: &HashMap<String, String>,
) -> Result<Option<DiligenceStudioConfig>, String> {
    let Some(value) = value(values, "DILIGENCE_STUDIO_API_BASE_URL") else {
        return Ok(None);
    };
    let mut base_url = reqwest::Url::parse(value)
        .map_err(|error| format!("invalid DILIGENCE_STUDIO_API_BASE_URL: {error}"))?;
    let is_loopback_http = base_url.scheme() == "http"
        && (base_url.host_str() == Some("localhost")
            || base_url
                .host_str()
                .and_then(|host| host.trim_matches(['[', ']']).parse::<IpAddr>().ok())
                .is_some_and(|address| address.is_loopback()));
    if base_url.scheme() != "https" && !is_loopback_http {
        return Err("DILIGENCE_STUDIO_API_BASE_URL must use HTTPS or loopback HTTP".to_string());
    }
    if !base_url.username().is_empty()
        || base_url.password().is_some()
        || base_url.query().is_some()
        || base_url.fragment().is_some()
    {
        return Err(
            "DILIGENCE_STUDIO_API_BASE_URL must not contain credentials, a query, or a fragment"
                .to_string(),
        );
    }
    if base_url.path().trim_end_matches('/') != "/api/v1" {
        return Err("DILIGENCE_STUDIO_API_BASE_URL must end with /api/v1".to_string());
    }
    base_url.set_path("/api/v1/");
    Ok(Some(DiligenceStudioConfig { base_url }))
}

fn parse_http_config(values: &HashMap<String, String>) -> Result<HttpConfig, String> {
    let host = value(values, "QUARRY_API_HOST").unwrap_or("127.0.0.1");
    let host = host
        .parse::<IpAddr>()
        .map_err(|error| format!("QUARRY_API_HOST must be an IP address: {error}"))?;
    let port = parse_value(values, "QUARRY_API_PORT", DEFAULT_API_PORT)?;
    let timeout_seconds = parse_value(
        values,
        "QUARRY_REQUEST_TIMEOUT_SECONDS",
        DEFAULT_REQUEST_TIMEOUT_SECONDS,
    )?;
    if timeout_seconds == 0 {
        return Err("QUARRY_REQUEST_TIMEOUT_SECONDS must be greater than zero".to_string());
    }

    let cors_origins = value(values, "QUARRY_CORS_ORIGINS")
        .unwrap_or(DEFAULT_CORS_ORIGINS)
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(|origin| {
            HeaderValue::from_str(origin)
                .map_err(|error| format!("invalid CORS origin `{origin}`: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if cors_origins.is_empty() {
        return Err("QUARRY_CORS_ORIGINS must include at least one origin".to_string());
    }

    Ok(HttpConfig {
        bind_address: SocketAddr::new(host, port),
        cors_origins,
        request_timeout: Duration::from_secs(timeout_seconds),
    })
}

fn parse_sqlite_config(values: &HashMap<String, String>) -> SqliteConfig {
    let path = value(values, "QUARRY_DATABASE_PATH")
        .map(PathBuf::from)
        .or_else(|| {
            value(values, "QUARRY_DATA_DIR")
                .map(|directory| PathBuf::from(directory).join(DEFAULT_DATABASE_FILE_NAME))
        })
        .unwrap_or_else(|| PathBuf::from("data").join(DEFAULT_DATABASE_FILE_NAME));
    SqliteConfig { path }
}

fn parse_helix_config(values: &HashMap<String, String>) -> Result<HelixConfig, String> {
    let url = value(values, "HELIX_URL")
        .unwrap_or(DEFAULT_HELIX_URL)
        .to_string();
    validate_helix_base_url(&url)?;
    let vector_dimension = parse_value(
        values,
        "HELIX_VECTOR_DIMENSION",
        DEFAULT_HELIX_VECTOR_DIMENSION,
    )?;
    let vector_dimension = NonZeroUsize::new(vector_dimension)
        .ok_or_else(|| "HELIX_VECTOR_DIMENSION must be greater than zero".to_string())?;
    Ok(HelixConfig {
        url,
        api_key: value(values, "HELIX_API_KEY").map(SecretString::new),
        vector_dimension,
    })
}

fn parse_openai_config(
    values: &HashMap<String, String>,
    embedding_dimensions: NonZeroUsize,
) -> Result<Option<OpenAiConfig>, String> {
    let names = [
        "OPENAI_API_KEY",
        "OPENAI_CHAT_MODEL",
        "OPENAI_DEAL_EXTRACTION_MODEL",
        "OPENAI_EMBEDDING_MODEL",
        "OPENAI_DOCUMENT_SUMMARY_MODEL",
        "OPENAI_IMAGE_DESCRIPTION_MODEL",
    ];
    if !names.iter().any(|name| value(values, name).is_some()) {
        return Ok(None);
    }
    let api_key = required(values, "OPENAI_API_KEY", "OpenAI capability")?;
    Ok(Some(OpenAiConfig {
        api_key: SecretString::new(api_key),
        chat_model: value(values, "OPENAI_CHAT_MODEL")
            .unwrap_or(DEFAULT_CHAT_MODEL)
            .to_string(),
        deal_extraction_model: value(values, "OPENAI_DEAL_EXTRACTION_MODEL")
            .unwrap_or(DEFAULT_DEAL_EXTRACTION_MODEL)
            .to_string(),
        embedding_model: value(values, "OPENAI_EMBEDDING_MODEL")
            .unwrap_or(DEFAULT_EMBEDDING_MODEL)
            .to_string(),
        embedding_dimensions,
        document_summary_model: value(values, "OPENAI_DOCUMENT_SUMMARY_MODEL")
            .unwrap_or(DEFAULT_DOCUMENT_SUMMARY_MODEL)
            .to_string(),
        image_description_model: value(values, "OPENAI_IMAGE_DESCRIPTION_MODEL")
            .unwrap_or(DEFAULT_IMAGE_DESCRIPTION_MODEL)
            .to_string(),
    }))
}

fn parse_wm_ai_config(values: &HashMap<String, String>) -> Result<Option<WmAiConfig>, String> {
    let names = [
        "WM_FILE_UPLOAD_SERVICE_URL",
        "WM_FILE_UPLOAD_API_KEY",
        "WM_INDEX_SERVICE_URL",
        "WM_INDEX_SERVICE_API_KEY",
        "WM_GRAPHRAG_URL",
        "WM_GRAPHRAG_API_KEY",
        "WM_GRAPHRAG_APPLICATION_NAME",
    ];
    if !names.iter().any(|name| value(values, name).is_some()) {
        return Ok(None);
    }
    let capability = "WM AI capability";
    let config = WmAiConfig {
        file_upload_url: required(values, names[0], capability)?,
        file_upload_api_key: SecretString::new(required(values, names[1], capability)?),
        index_url: required(values, names[2], capability)?,
        index_api_key: SecretString::new(required(values, names[3], capability)?),
        graph_rag_url: required(values, names[4], capability)?,
        graph_rag_api_key: SecretString::new(required(values, names[5], capability)?),
        graph_rag_application_name: required(values, names[6], capability)?,
    };
    validate_url(names[0], &config.file_upload_url)?;
    validate_url(names[2], &config.index_url)?;
    validate_url(names[4], &config.graph_rag_url)?;
    Ok(Some(config))
}

fn parse_data_room_config(values: &HashMap<String, String>) -> DataRoomConfig {
    let roots = values
        .iter()
        .filter_map(|(name, path)| {
            name.strip_prefix("QUARRY_DATA_ROOM_")
                .filter(|deal| !deal.is_empty())
                .and_then(|deal| {
                    let path = path.trim();
                    (!path.is_empty()).then(|| (deal.to_string(), PathBuf::from(path)))
                })
        })
        .collect();
    DataRoomConfig {
        roots,
        office_executable: resolve_office_executable(values),
    }
}

fn resolve_office_executable(values: &HashMap<String, String>) -> Option<PathBuf> {
    if let Some(configured) = value(values, "QUARRY_SOFFICE").map(PathBuf::from) {
        return Some(configured);
    }
    if let Some(path) = values.get("PATH") {
        if let Some(executable) = env::split_paths(path)
            .flat_map(|directory| [directory.join("soffice"), directory.join("libreoffice")])
            .find(|candidate| candidate.is_file())
        {
            return Some(executable);
        }
    }
    [
        "/Applications/LibreOffice.app/Contents/MacOS/soffice",
        "/Applications/LibreOfficeDev.app/Contents/MacOS/soffice",
        "/opt/homebrew/bin/soffice",
        "/usr/local/bin/soffice",
        "/usr/bin/soffice",
        "/usr/bin/libreoffice",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|candidate| candidate.is_file())
}

fn parse_document_config(values: &HashMap<String, String>) -> Result<DocumentConfig, String> {
    let max_concurrent_documents = parse_value(
        values,
        "QUARRY_DOCUMENT_CONCURRENCY",
        DEFAULT_DOCUMENT_CONCURRENCY,
    )?;
    if max_concurrent_documents == 0 {
        return Err("QUARRY_DOCUMENT_CONCURRENCY must be greater than zero".to_string());
    }
    let retention_seconds = parse_value(
        values,
        "QUARRY_COMPLETED_JOB_RETENTION_SECONDS",
        DEFAULT_COMPLETED_JOB_RETENTION_SECONDS,
    )?;
    Ok(DocumentConfig {
        max_concurrent_documents,
        completed_job_retention: Duration::from_secs(retention_seconds),
    })
}

fn value<'a>(values: &'a HashMap<String, String>, name: &str) -> Option<&'a str> {
    values
        .get(name)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn required(
    values: &HashMap<String, String>,
    name: &str,
    capability: &str,
) -> Result<String, String> {
    value(values, name)
        .map(ToString::to_string)
        .ok_or_else(|| format!("{capability} is partially configured: {name} is required"))
}

fn parse_value<T>(values: &HashMap<String, String>, name: &str, default: T) -> Result<T, String>
where
    T: std::str::FromStr,
    T::Err: fmt::Display,
{
    match value(values, name) {
        Some(value) => value
            .parse::<T>()
            .map_err(|error| format!("invalid {name}: {error}")),
        None => Ok(default),
    }
}

fn validate_url(name: &str, value: &str) -> Result<(), String> {
    reqwest::Url::parse(value)
        .map(|_| ())
        .map_err(|error| format!("invalid {name}: {error}"))
}

fn validate_helix_base_url(value: &str) -> Result<(), String> {
    let parsed =
        reqwest::Url::parse(value).map_err(|error| format!("invalid HELIX_URL: {error}"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err("HELIX_URL must be an HTTP(S) instance base URL".to_string());
    }
    if !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.path() != "/"
    {
        return Err(
            "HELIX_URL must be an instance base URL without a path, credentials, query, or fragment"
                .to_string(),
        );
    }
    Ok(())
}

fn normalize_deal_config_key(deal_id: &str) -> String {
    deal_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "../../tests/unit/app/config_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TemplateBackend {
    DiligenceStudio,
    #[default]
    Embedded,
}
#[derive(Clone, Debug)]
pub struct SlideTemplatesConfig {
    pub backend: TemplateBackend,
    pub embedding_dimensions: NonZeroUsize,
    pub chromium_path: Option<PathBuf>,
    pub render_url: String,
    pub preview_max_bytes: usize,
    pub preview_render_size: u32,
    pub preview_timeout: Duration,
    pub classification: Option<SlideClassificationConfig>,
    pub allow_embedding_inspection: bool,
}
#[derive(Clone, Debug)]
pub struct SlideClassificationConfig {
    pub key: SecretString,
    pub model: String,
    pub embedding_model: String,
    pub dimensions: NonZeroUsize,
    pub timeout: Duration,
    pub max_output_tokens: usize,
    pub max_embedding_text_bytes: usize,
}
fn parse_slide_templates_config(
    values: &HashMap<String, String>,
) -> Result<SlideTemplatesConfig, String> {
    let backend = match value(values, "QUARRY_TEMPLATE_BACKEND").unwrap_or("embedded") {
        "diligence-studio" => TemplateBackend::DiligenceStudio,
        "embedded" => TemplateBackend::Embedded,
        _ => return Err("QUARRY_TEMPLATE_BACKEND must be diligence-studio or embedded".into()),
    };
    let render_url = value(values, "QUARRY_TEMPLATE_PREVIEW_RENDER_URL")
        .unwrap_or("http://localhost:1420/_internal/template-preview")
        .to_owned();
    let url =
        reqwest::Url::parse(&render_url).map_err(|_| "invalid template preview render URL")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err("invalid template preview render URL".into());
    }
    let preview_max_bytes =
        parse_value(values, "MAX_TEMPLATE_PREVIEW_BYTES", 10 * 1024 * 1024usize)?;
    let preview_render_size = parse_value(values, "TEMPLATE_PREVIEW_RENDER_SIZE", 1600u32)?;
    let preview_timeout_ms = parse_value(values, "TEMPLATE_PREVIEW_TIMEOUT_MS", 15_000u64)?;
    if !(1..=16 * 1024 * 1024).contains(&preview_max_bytes)
        || !(320..=4096).contains(&preview_render_size)
        || !(1..=120_000).contains(&preview_timeout_ms)
    {
        return Err("template preview limits are outside supported ranges".into());
    }
    let chromium_path = match value(values, "QUARRY_SLIDE_TEMPLATES_PREVIEW").unwrap_or("chromium")
    {
        "disabled" => None,
        "chromium" => resolve_chromium(values)?,
        _ => return Err("QUARRY_SLIDE_TEMPLATES_PREVIEW must be disabled or chromium".into()),
    };
    let dimensions = parse_value(values, "OPENAI_SLIDE_EMBEDDING_DIMENSIONS", 1536usize)?;
    if !(1..=16_384).contains(&dimensions) {
        return Err("OPENAI_SLIDE_EMBEDDING_DIMENSIONS must be in 1..=16384".into());
    }
    let embedding_dimensions =
        NonZeroUsize::new(dimensions).ok_or("invalid slide embedding dimensions")?;
    let classification = match value(values, "QUARRY_SLIDE_CLASSIFICATION_PROVIDER")
        .unwrap_or("disabled")
    {
        "disabled" => None,
        "openai" => {
            let group = "slide classification";
            let timeout = parse_value(values, "OPENAI_SLIDE_TIMEOUT_MS", 30_000u64)?;
            let max_output_tokens =
                parse_value(values, "OPENAI_SLIDE_MAX_OUTPUT_TOKENS", 4096usize)?;
            let max_embedding_text_bytes =
                parse_value(values, "OPENAI_SLIDE_EMBEDDING_MAX_TEXT_BYTES", 32_000usize)?;
            if dimensions == 0
                || dimensions > 16_384
                || timeout == 0
                || timeout > 120_000
                || max_output_tokens == 0
                || max_output_tokens > 32_768
                || max_embedding_text_bytes == 0
                || max_embedding_text_bytes > 128_000
            {
                return Err("slide classification limits are outside supported ranges".into());
            }
            Some(SlideClassificationConfig {
                key: SecretString::new(required(values, "OPENAI_API_KEY", group)?),
                model: required(values, "OPENAI_SLIDE_CLASSIFICATION_MODEL", group)?,
                embedding_model: required(values, "OPENAI_SLIDE_EMBEDDING_MODEL", group)?,
                dimensions: embedding_dimensions,
                timeout: Duration::from_millis(timeout),
                max_output_tokens,
                max_embedding_text_bytes,
            })
        }
        _ => return Err("QUARRY_SLIDE_CLASSIFICATION_PROVIDER must be disabled or openai".into()),
    };
    let allow_embedding_inspection =
        match value(values, "QUARRY_SLIDE_TEMPLATES_ALLOW_EMBEDDING_INSPECTION").unwrap_or("false")
        {
            "true" => true,
            "false" => false,
            _ => {
                return Err(
                    "QUARRY_SLIDE_TEMPLATES_ALLOW_EMBEDDING_INSPECTION must be true or false"
                        .into(),
                )
            }
        };
    Ok(SlideTemplatesConfig {
        backend,
        embedding_dimensions,
        chromium_path,
        render_url,
        preview_max_bytes,
        preview_render_size,
        preview_timeout: Duration::from_millis(preview_timeout_ms),
        classification,
        allow_embedding_inspection,
    })
}
fn resolve_chromium(values: &HashMap<String, String>) -> Result<Option<PathBuf>, String> {
    if let Some(path) = value(values, "QUARRY_SLIDE_TEMPLATES_CHROMIUM_PATH") {
        let path = PathBuf::from(path);
        if !path.is_absolute() || !path.is_file() {
            return Err(
                "QUARRY_SLIDE_TEMPLATES_CHROMIUM_PATH must name an existing absolute executable"
                    .into(),
            );
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if std::fs::metadata(&path)
                .map_err(|_| "could not inspect Chromium executable")?
                .permissions()
                .mode()
                & 0o111
                == 0
            {
                return Err("QUARRY_SLIDE_TEMPLATES_CHROMIUM_PATH must be executable".into());
            }
        }
        return Ok(Some(path));
    }
    let mut candidates = Vec::new();
    if let Some(home) = value(values, "HOME") {
        for cache in [
            PathBuf::from(home).join("Library/Caches/ms-playwright"),
            PathBuf::from(home).join(".cache/ms-playwright"),
        ] {
            if let Ok(entries) = std::fs::read_dir(cache) {
                let mut entries = entries.filter_map(Result::ok).collect::<Vec<_>>();
                entries.sort_by_key(|e| std::cmp::Reverse(e.file_name()));
                for entry in entries {
                    for relative in ["chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing","chrome-mac/Chromium.app/Contents/MacOS/Chromium","chrome-linux64/chrome","chrome-linux/chrome","chrome-headless-shell-mac-arm64/chrome-headless-shell","chrome-headless-shell-linux64/chrome-headless-shell"] {candidates.push(entry.path().join(relative));}
                }
            }
        }
    }
    candidates.extend(
        [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/usr/bin/chromium",
            "/usr/bin/chromium-browser",
            "/usr/bin/google-chrome",
        ]
        .into_iter()
        .map(PathBuf::from),
    );
    Ok(candidates.into_iter().find(|p| p.is_file()))
}
