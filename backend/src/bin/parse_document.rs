use base64::{engine::general_purpose, Engine as _};
use quarry_backend::{
    config::AppConfig,
    core::{
        clients::openai::{OpenAiClient, ResponsesFileInput},
        parsers::docx::parse_docx_from_path,
        prompts::{build_basic_document_summary_prompt, CLI_DOCUMENT_SUMMARY_SYSTEM_PROMPT},
        CollectedFile,
    },
};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process,
};
use walkdir::WalkDir;

const APP_NAME: &str = "DataRoomCLI";
const MAX_FILE_BYTES: usize = 50 * 1024 * 1024;
const MAX_TOTAL_REQUEST_FILE_BYTES: usize = 50 * 1024 * 1024;
#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();
    let _ = tracing_subscriber::fmt()
        .with_env_filter("quarry_backend=info")
        .try_init();
    let args: Vec<String> = env::args().collect();

    let result = match args.get(1).map(String::as_str) {
        None | Some("-h") | Some("--help") | Some("help") => {
            print_help();
            Ok(())
        }
        Some("-V") | Some("--version") | Some("version") => {
            println!("{APP_NAME} {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("docx") => match args.get(2) {
            Some(path) => parse_docx(path.to_string()).await,
            None => Err("missing docx path".to_string()),
        },
        Some("summarize") => match args.get(2) {
            Some(path) => summarize_dir(path.to_string()).await,
            None => Err("missing directory path".to_string()),
        },
        Some(command) => Err(format!("unknown command: {command}")),
    };

    if let Err(message) = result {
        eprintln!("error: {message}");
        eprintln!("run `dataroomcli help` for usage");
        process::exit(1);
    }
}

fn print_help() {
    println!(
        "{APP_NAME}

Usage:
  dataroomcli <command> [options]

Commands:
  docx <path>                          Parse a DOCX file
  summarize <directory>               Summarize supported files in a directory with OpenAI
  upload <directory>                  Alias for summarize
  dir <directory>                     Alias for summarize
  help                                Show this help message
  version                             Show the current version
"
    );
}

async fn parse_docx(path: String) -> Result<(), String> {
    let path: &Path = Path::new(&path);
    let text = parse_docx_from_path(path)?;
    println!("{text}");
    Ok(())
}

async fn summarize_dir(path: String) -> Result<(), String> {
    let root = PathBuf::from(&path);

    if !root.exists() {
        return Err(format!("path does not exist: {}", root.display()));
    }

    if !root.is_dir() {
        return Err(format!("path is not a directory: {}", root.display()));
    }

    let (files, skipped_files) = collect_supported_files(&root)?;
    if files.is_empty() {
        return Err(format!("no supported files found in {}", root.display()));
    }

    let config = AppConfig::from_env()?;
    let openai = config
        .openai
        .as_ref()
        .ok_or_else(|| "OpenAI capability is not configured".to_string())?;
    let client = OpenAiClient::from_config(reqwest::Client::new(), openai);
    let prompt = build_basic_document_summary_prompt(&root, &files, &skipped_files);
    let file_inputs: Vec<ResponsesFileInput<'_>> = files
        .iter()
        .map(|file| ResponsesFileInput::FileData {
            filename: file.filename.as_str(),
            mime_type: file.mime_type,
            data_base64: file.data_base64.as_str(),
        })
        .collect();

    let summary = client
        .gen_model_response_with_files(
            Some(&prompt),
            Some(CLI_DOCUMENT_SUMMARY_SYSTEM_PROMPT),
            Some(&openai.document_summary_model),
            Some(&file_inputs),
        )
        .await?;

    println!("{summary}");
    write_summary(&summary)?;

    if !skipped_files.is_empty() {
        eprintln!("skipped {} unsupported or empty files", skipped_files.len());
    }

    Ok(())
}

fn collect_supported_files(root: &Path) -> Result<(Vec<CollectedFile>, Vec<String>), String> {
    let mut files = Vec::new();
    let mut skipped_files = Vec::new();
    let mut total_file_bytes = 0usize;
    let mut total_limit_reached = false;

    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.into_path();
        let relative_path = display_relative_path(root, &path);
        let Some(mime_type) = infer_supported_mime_type(&path) else {
            skipped_files.push(relative_path);
            continue;
        };
        println!("{}", path.display());

        if total_limit_reached {
            skipped_files.push(format!(
                "{relative_path} (skipped: total request file size limit already reached)"
            ));
            continue;
        }

        let file_size_bytes = fs::metadata(&path)
            .map_err(|err| format!("failed to read metadata for {}: {err}", path.display()))?
            .len() as usize;
        if file_size_bytes == 0 {
            skipped_files.push(format!("{relative_path} (empty)"));
            continue;
        }

        if file_size_bytes > MAX_FILE_BYTES {
            skipped_files.push(format!(
                "{relative_path} (skipped: file exceeds 50 MB limit)"
            ));
            continue;
        }

        if total_file_bytes + file_size_bytes > MAX_TOTAL_REQUEST_FILE_BYTES {
            skipped_files.push(format!(
                "{relative_path} (skipped: total request file size would exceed 50 MB limit)"
            ));
            total_limit_reached = true;
            continue;
        }

        let file_bytes =
            fs::read(&path).map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        if file_bytes.is_empty() {
            skipped_files.push(format!("{relative_path} (empty)"));
            continue;
        }

        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("failed to derive filename from {}", path.display()))?
            .to_string();

        files.push(CollectedFile {
            filename,
            relative_path,
            mime_type,
            size_bytes: file_size_bytes,
            data_base64: general_purpose::STANDARD.encode(file_bytes),
        });
        total_file_bytes += file_size_bytes;
    }

    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    skipped_files.sort();

    Ok((files, skipped_files))
}

fn display_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn infer_supported_mime_type(path: &Path) -> Option<&'static str> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("pdf") => Some("application/pdf"),
        Some("txt") => Some("text/plain"),
        Some("md") => Some("text/markdown"),
        Some("json") => Some("application/json"),
        Some("html") => Some("text/html"),
        Some("csv") => Some("text/csv"),
        Some("doc") => Some("application/msword"),
        Some("docx") => {
            Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
        }
        Some("pptx") => {
            Some("application/vnd.openxmlformats-officedocument.presentationml.presentation")
        }
        Some("xls") => Some("application/vnd.ms-excel"),
        Some("xlsx") => Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        _ => None,
    }
}

fn write_summary(summary: &str) -> Result<(), String> {
    let output_path = Path::new("../../output-text.md");
    fs::write(output_path, summary)
        .map_err(|err| format!("failed to write {}: {err}", output_path.display()))?;
    println!("wrote summary to {}", output_path.display());
    Ok(())
}
