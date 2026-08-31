use std::{
    collections::HashMap,
    env, fmt,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    time::Duration,
};

use axum::http::HeaderValue;

const DEFAULT_API_PORT: u16 = 3001;
const DEFAULT_REQUEST_TIMEOUT_SECONDS: u64 = 120;
const DEFAULT_CORS_ORIGINS: &str = "http://127.0.0.1:1420,http://localhost:1420";
const DEFAULT_DATABASE_FILE_NAME: &str = "quarry.sqlite3";
const DEFAULT_HELIX_URL: &str = "http://127.0.0.1:6969";
const DEFAULT_DEAL_EXTRACTION_MODEL: &str = "gpt-5.6-luna";
const DEFAULT_EMBEDDING_MODEL: &str = "text-embedding-3-small";
const DEFAULT_DOCUMENT_SUMMARY_MODEL: &str = "gpt-5.5";
const DEFAULT_IMAGE_DESCRIPTION_MODEL: &str = "gpt-5.5";
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
}

#[derive(Clone, Debug)]
pub struct OpenAiConfig {
    pub api_key: SecretString,
    pub deal_extraction_model: String,
    pub embedding_model: String,
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
pub struct AppConfig {
    pub http: HttpConfig,
    pub sqlite: SqliteConfig,
    pub helix: HelixConfig,
    pub openai: Option<OpenAiConfig>,
    pub wm_ai: Option<WmAiConfig>,
    pub data_room: DataRoomConfig,
    pub documents: DocumentConfig,
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

        Ok(Self {
            http: parse_http_config(&values)?,
            sqlite: parse_sqlite_config(&values),
            helix: parse_helix_config(&values)?,
            openai: parse_openai_config(&values)?,
            wm_ai: parse_wm_ai_config(&values)?,
            data_room: parse_data_room_config(&values),
            documents: parse_document_config(&values)?,
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
        }
    }
}

fn parse_http_config(values: &HashMap<String, String>) -> Result<HttpConfig, String> {
    let host = value(values, "PATHFINDER_API_HOST").unwrap_or("127.0.0.1");
    let host = host
        .parse::<IpAddr>()
        .map_err(|error| format!("PATHFINDER_API_HOST must be an IP address: {error}"))?;
    let port = parse_value(values, "PATHFINDER_API_PORT", DEFAULT_API_PORT)?;
    let timeout_seconds = parse_value(
        values,
        "PATHFINDER_REQUEST_TIMEOUT_SECONDS",
        DEFAULT_REQUEST_TIMEOUT_SECONDS,
    )?;
    if timeout_seconds == 0 {
        return Err("PATHFINDER_REQUEST_TIMEOUT_SECONDS must be greater than zero".to_string());
    }

    let cors_origins = value(values, "PATHFINDER_CORS_ORIGINS")
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
        return Err("PATHFINDER_CORS_ORIGINS must include at least one origin".to_string());
    }

    Ok(HttpConfig {
        bind_address: SocketAddr::new(host, port),
        cors_origins,
        request_timeout: Duration::from_secs(timeout_seconds),
    })
}

fn parse_sqlite_config(values: &HashMap<String, String>) -> SqliteConfig {
    let path = value(values, "PATHFINDER_DATABASE_PATH")
        .map(PathBuf::from)
        .or_else(|| {
            value(values, "PATHFINDER_DATA_DIR")
                .map(|directory| PathBuf::from(directory).join(DEFAULT_DATABASE_FILE_NAME))
        })
        .unwrap_or_else(|| PathBuf::from("data").join(DEFAULT_DATABASE_FILE_NAME));
    SqliteConfig { path }
}

fn parse_helix_config(values: &HashMap<String, String>) -> Result<HelixConfig, String> {
    let url = value(values, "HELIX_URL")
        .unwrap_or(DEFAULT_HELIX_URL)
        .to_string();
    validate_url("HELIX_URL", &url)?;
    Ok(HelixConfig {
        url,
        api_key: value(values, "HELIX_API_KEY").map(SecretString::new),
    })
}

fn parse_openai_config(values: &HashMap<String, String>) -> Result<Option<OpenAiConfig>, String> {
    let names = [
        "OPENAI_API_KEY",
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
        deal_extraction_model: value(values, "OPENAI_DEAL_EXTRACTION_MODEL")
            .unwrap_or(DEFAULT_DEAL_EXTRACTION_MODEL)
            .to_string(),
        embedding_model: value(values, "OPENAI_EMBEDDING_MODEL")
            .unwrap_or(DEFAULT_EMBEDDING_MODEL)
            .to_string(),
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
#[path = "../tests/config_tests.rs"]
mod tests;
