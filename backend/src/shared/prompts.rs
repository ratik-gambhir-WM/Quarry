use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use thiserror::Error;

const SCHEMA_VERSION: &str = "1";
const CATALOG_ASSETS: &[&str] = &[
    "document_summary.md",
    "company_research.md",
    "diligence_review.md",
    "system/research_agent.md",
];
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PromptId {
    DefaultResponse,
    DefaultInstructions,
    HelixQueryExample,
    DealExtractionSystem,
    DealExtractionRequest,
    SummarySystem,
    CliSummarySystem,
    BasicSummaryRequest,
    TechnologyDiligence,
    ProductApplicationDeepDive,
    ImageDescription,
    AssistantChatDefault,
}

impl PromptId {
    pub const ALL: [Self; 12] = [
        Self::DefaultResponse,
        Self::DefaultInstructions,
        Self::HelixQueryExample,
        Self::DealExtractionSystem,
        Self::DealExtractionRequest,
        Self::SummarySystem,
        Self::CliSummarySystem,
        Self::BasicSummaryRequest,
        Self::TechnologyDiligence,
        Self::ProductApplicationDeepDive,
        Self::ImageDescription,
        Self::AssistantChatDefault,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DefaultResponse => "default-response",
            Self::DefaultInstructions => "default-instructions",
            Self::HelixQueryExample => "helix-query-example",
            Self::DealExtractionSystem => "deal-extraction-system",
            Self::DealExtractionRequest => "deal-extraction-request",
            Self::SummarySystem => "summary-system",
            Self::CliSummarySystem => "cli-summary-system",
            Self::BasicSummaryRequest => "basic-summary-request",
            Self::TechnologyDiligence => "technology-diligence",
            Self::ProductApplicationDeepDive => "product-application-deep-dive",
            Self::ImageDescription => "image-description",
            Self::AssistantChatDefault => "assistant-chat-default",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|id| id.as_str() == value)
    }

    const fn asset(self) -> &'static str {
        match self {
            Self::DefaultResponse
            | Self::DefaultInstructions
            | Self::ImageDescription
            | Self::AssistantChatDefault => "system/research_agent.md",
            Self::HelixQueryExample => "company_research.md",
            Self::DealExtractionSystem | Self::DealExtractionRequest => "diligence_review.md",
            Self::SummarySystem
            | Self::CliSummarySystem
            | Self::BasicSummaryRequest
            | Self::TechnologyDiligence
            | Self::ProductApplicationDeepDive => "document_summary.md",
        }
    }

    const fn allowed_variables(self) -> &'static [&'static str] {
        match self {
            Self::DealExtractionRequest => &[
                "deal_id",
                "deal_name",
                "transaction_type",
                "target_company",
                "primary_buyer",
                "deal_sponsor",
                "attached_file_manifest",
            ],
            Self::BasicSummaryRequest => &["root", "document_manifest", "skipped_files"],
            _ => &[],
        }
    }
}

pub struct DealExtractionPromptData<'a> {
    pub deal_id: &'a str,
    pub deal_name: &'a str,
    pub transaction_type: &'a str,
    pub target_company: &'a str,
    pub primary_buyer: &'a str,
    pub deal_sponsor: &'a str,
    pub attached_file_manifest: &'a str,
}

#[derive(Clone, Debug)]
pub struct PromptCatalog {
    prompts: BTreeMap<PromptId, String>,
}

impl PromptCatalog {
    pub fn load(directory: &Path) -> Result<Self, PromptCatalogError> {
        let mut prompts = BTreeMap::new();
        for asset in CATALOG_ASSETS {
            let path = directory.join(asset);
            let contents =
                fs::read_to_string(path).map_err(|error| PromptCatalogError::ReadAsset {
                    asset,
                    kind: error.kind(),
                })?;
            for entry in parse_asset(asset, &contents)? {
                let id =
                    PromptId::from_str(&entry.id).ok_or_else(|| PromptCatalogError::Invalid {
                        asset,
                        detail: format!("unknown prompt id `{}`", entry.id),
                    })?;
                if id.asset() != *asset {
                    return Err(PromptCatalogError::Invalid {
                        asset,
                        detail: format!("prompt id `{}` belongs in `{}`", id.as_str(), id.asset()),
                    });
                }
                validate_entry(asset, id, &entry)?;
                if prompts.insert(id, entry.body.clone()).is_some() {
                    return Err(PromptCatalogError::Invalid {
                        asset,
                        detail: format!("duplicate prompt id `{}`", id.as_str()),
                    });
                }
            }
        }

        for id in PromptId::ALL {
            if !prompts.contains_key(&id) {
                return Err(PromptCatalogError::Invalid {
                    asset: id.asset(),
                    detail: format!("required prompt id `{}` is missing", id.as_str()),
                });
            }
        }

        Ok(Self { prompts })
    }

    pub fn prompt(&self, id: PromptId) -> &str {
        self.prompts
            .get(&id)
            .map(String::as_str)
            .expect("catalog validation loads every typed prompt")
    }

    pub fn render_deal_extraction(&self, data: DealExtractionPromptData<'_>) -> String {
        render_prompt(
            self.prompt(PromptId::DealExtractionRequest),
            &[
                ("deal_id", data.deal_id),
                ("deal_name", data.deal_name),
                ("transaction_type", data.transaction_type),
                ("target_company", data.target_company),
                ("primary_buyer", data.primary_buyer),
                ("deal_sponsor", data.deal_sponsor),
                ("attached_file_manifest", data.attached_file_manifest),
            ],
        )
    }

    pub fn render_basic_summary(
        &self,
        root: &str,
        document_manifest: &str,
        skipped_files: &str,
    ) -> String {
        render_prompt(
            self.prompt(PromptId::BasicSummaryRequest),
            &[
                ("root", root),
                ("document_manifest", document_manifest),
                ("skipped_files", skipped_files),
            ],
        )
    }
}

#[derive(Debug, Error)]
pub enum PromptCatalogError {
    #[error("prompt catalog asset `{asset}` could not be read ({kind:?})")]
    ReadAsset {
        asset: &'static str,
        kind: std::io::ErrorKind,
    },
    #[error("prompt catalog asset `{asset}` is invalid: {detail}")]
    Invalid { asset: &'static str, detail: String },
}

#[derive(Clone, Debug)]
struct ParsedEntry {
    id: String,
    body: String,
}

fn parse_asset(
    asset: &'static str,
    contents: &str,
) -> Result<Vec<ParsedEntry>, PromptCatalogError> {
    let lines = contents.lines().collect::<Vec<_>>();
    if lines.first().copied() != Some("---") {
        return invalid(asset, "front matter must begin with `---`");
    }
    let Some(front_matter_end) = lines
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(index, line)| (*line == "---").then_some(index))
    else {
        return invalid(asset, "front matter closing delimiter is missing");
    };
    validate_front_matter(asset, &lines[1..front_matter_end])?;

    let mut entries = Vec::new();
    let mut current: Option<ParsedEntry> = None;
    let mut collecting_body = false;
    let mut body_lines = Vec::new();
    let mut body_closed = false;

    for line in &lines[front_matter_end + 1..] {
        if collecting_body {
            if *line == "```" {
                let entry = current.as_mut().expect("fence requires a prompt entry");
                entry.body = body_lines.join("\n").trim().to_string();
                collecting_body = false;
                body_closed = true;
            } else {
                body_lines.push(*line);
            }
            continue;
        }

        if let Some(id) = line.strip_prefix("## ") {
            if let Some(entry) = current.take() {
                if !body_closed {
                    return invalid(
                        asset,
                        "every prompt entry must contain one closed `text` fence",
                    );
                }
                entries.push(entry);
            }
            validate_id(asset, id)?;
            current = Some(ParsedEntry {
                id: id.to_string(),
                body: String::new(),
            });
            body_lines.clear();
            body_closed = false;
            continue;
        }

        if current.is_none() {
            continue;
        }
        if *line == "```text" {
            if body_closed {
                return invalid(asset, "a prompt entry may contain only one `text` fence");
            }
            collecting_body = true;
            body_lines.clear();
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        return invalid(asset, "prompt entries may contain only one `text` fence");
    }

    if collecting_body {
        return invalid(asset, "a prompt `text` fence is not closed");
    }
    if let Some(entry) = current {
        if !body_closed {
            return invalid(
                asset,
                "every prompt entry must contain one closed `text` fence",
            );
        }
        entries.push(entry);
    }
    if entries.is_empty() {
        return invalid(asset, "at least one prompt entry is required");
    }
    Ok(entries)
}

fn validate_front_matter(asset: &'static str, lines: &[&str]) -> Result<(), PromptCatalogError> {
    let mut values = BTreeMap::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            return invalid(asset, "front matter must use `key: value` syntax");
        };
        let key = key.trim();
        let value = value.trim();
        if value.is_empty() || values.insert(key.to_string(), value.to_string()).is_some() {
            return invalid(asset, "front matter values must be nonempty and unique");
        }
    }
    if values.get("schemaVersion").map(String::as_str) != Some(SCHEMA_VERSION) {
        return invalid(asset, "schemaVersion must be 1");
    }
    for required in ["title", "owner"] {
        if !values.contains_key(required) {
            return invalid(
                asset,
                "front matter requires schemaVersion, title, and owner",
            );
        }
    }
    Ok(())
}

fn validate_entry(
    asset: &'static str,
    id: PromptId,
    entry: &ParsedEntry,
) -> Result<(), PromptCatalogError> {
    if entry.body.is_empty() {
        return invalid(asset, "prompt bodies must be nonempty");
    }
    let variables = parse_variables(asset, &entry.body)?;
    let expected = id
        .allowed_variables()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let actual = variables
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(PromptCatalogError::Invalid {
            asset,
            detail: format!(
                "prompt id `{}` has unsupported or missing template variables",
                id.as_str()
            ),
        });
    }
    Ok(())
}

fn parse_variables(asset: &'static str, body: &str) -> Result<Vec<String>, PromptCatalogError> {
    let mut variables = Vec::new();
    let mut remaining = body;
    while let Some(start) = remaining.find("{{") {
        if remaining[..start].contains("}}") {
            return invalid(asset, "template variable delimiters are malformed");
        }
        let after_open = &remaining[start + 2..];
        let Some(end) = after_open.find("}}") else {
            return invalid(asset, "template variable delimiters are malformed");
        };
        let variable = &after_open[..end];
        if variable.is_empty()
            || !variable
                .chars()
                .all(|character| character.is_ascii_lowercase() || character == '_')
            || variables.iter().any(|existing| existing == variable)
        {
            return invalid(
                asset,
                "template variables must be unique lowercase identifiers",
            );
        }
        variables.push(variable.to_string());
        remaining = &after_open[end + 2..];
    }
    if remaining.contains("}}") {
        return invalid(asset, "template variable delimiters are malformed");
    }
    Ok(variables)
}

fn validate_id(asset: &'static str, id: &str) -> Result<(), PromptCatalogError> {
    if id.is_empty()
        || !id.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
    {
        return invalid(asset, "prompt ids must use lowercase kebab case");
    }
    Ok(())
}

fn render_prompt(template: &str, values: &[(&str, &str)]) -> String {
    let mut rendered = String::with_capacity(template.len());
    let mut remaining = template;

    while let Some(start) = remaining.find("{{") {
        rendered.push_str(&remaining[..start]);
        let after_open = &remaining[start + 2..];
        let end = after_open
            .find("}}")
            .expect("catalog validation requires closed template variables");
        let name = &after_open[..end];
        let value = values
            .iter()
            .find_map(|(candidate, value)| (*candidate == name).then_some(*value))
            .expect("catalog validation requires every template variable to be rendered");
        rendered.push_str(value);
        remaining = &after_open[end + 2..];
    }

    rendered.push_str(remaining);
    rendered
}

fn invalid<T>(asset: &'static str, detail: impl Into<String>) -> Result<T, PromptCatalogError> {
    Err(PromptCatalogError::Invalid {
        asset,
        detail: detail.into(),
    })
}

#[cfg(test)]
#[path = "../../tests/unit/shared/prompts_tests.rs"]
mod tests;
