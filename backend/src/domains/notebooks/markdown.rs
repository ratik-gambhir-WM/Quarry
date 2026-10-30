//! Versioned, deterministic, lossy Markdown projection. Native blocks stay canonical.
use super::model::Note;
use serde_json::Value;

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if "\\`*_{}[]<>#+-.!|".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
fn inline(value: &Value, plain: bool) -> String {
    let mut result = String::new();
    if let Some(items) = value.as_array() {
        for item in items {
            if item["type"] == "link" {
                let text = inline(&item["content"], false);
                let href = item["href"]
                    .as_str()
                    .unwrap_or_default()
                    .replace('(', "%28")
                    .replace(')', "%29");
                result.push_str(&format!("[{text}](<{href}>)"));
            } else {
                let text = item["text"].as_str().unwrap_or_default();
                let mut text = if plain { text.to_owned() } else { escape(text) };
                if !plain {
                    for (key, delimiter) in [
                        ("code", "`"),
                        ("bold", "**"),
                        ("italic", "*"),
                        ("strike", "~~"),
                    ] {
                        if item["styles"][key] == true {
                            text = format!("{delimiter}{text}{delimiter}");
                        }
                    }
                }
                result.push_str(&text);
            }
        }
    }
    result
}
fn blocks(value: &Value, depth: usize, out: &mut String) {
    let Some(items) = value.as_array() else {
        return;
    };
    for block in items {
        let kind = block["type"].as_str().unwrap_or_default();
        let text = inline(&block["content"], kind == "codeBlock");
        let indent = "  ".repeat(depth);
        let line = match kind {
            "heading" => format!(
                "{} {text}",
                "#".repeat(
                    usize::try_from(block["props"]["level"].as_u64().unwrap_or(1)).unwrap_or(1)
                )
            ),
            "bulletListItem" => format!("{indent}- {text}"),
            "numberedListItem" => format!(
                "{indent}{}. {text}",
                block["props"]["start"].as_u64().unwrap_or(1)
            ),
            "checkListItem" => format!(
                "{indent}- [{}] {text}",
                if block["props"]["checked"] == true {
                    "x"
                } else {
                    " "
                }
            ),
            "quote" => format!("> {}", text.replace('\n', "\n> ")),
            "codeBlock" => {
                let fence = "`".repeat(
                    text.split(|c| c != '`')
                        .map(str::len)
                        .max()
                        .unwrap_or(0)
                        .max(2)
                        + 1,
                );
                let language = block["props"]["language"].as_str().unwrap_or("text");
                format!("{fence}{language}\n{text}\n{fence}")
            }
            "table" => {
                let mut table = String::new();
                if let Some(rows) = block["content"]["rows"].as_array() {
                    for (index, row) in rows.iter().enumerate() {
                        if let Some(cells) = row["cells"].as_array() {
                            let cells_text = cells
                                .iter()
                                .map(|cell| {
                                    inline(
                                        if cell.is_array() {
                                            cell
                                        } else {
                                            &cell["content"]
                                        },
                                        false,
                                    )
                                    .replace('\n', " ")
                                })
                                .collect::<Vec<_>>();
                            table.push_str(&format!("| {} |\n", cells_text.join(" | ")));
                            if index == 0 {
                                table.push_str(&format!(
                                    "| {} |\n",
                                    vec!["---"; cells.len()].join(" | ")
                                ));
                            }
                        }
                    }
                }
                table
            }
            _ => text,
        };
        out.push_str(&line);
        out.push_str("\n\n");
        blocks(&block["children"], depth + 1, out);
    }
}
pub(super) fn export(note: &Note) -> Result<String, &'static str> {
    let mut out = String::new();
    if !note.summary.title.is_empty() {
        out.push_str(&format!("# {}\n\n", escape(&note.summary.title)));
    }
    blocks(&note.blocks, 0, &mut out);
    if out.len() > 2 * 1024 * 1024 {
        return Err("export_limit");
    }
    Ok(out)
}
/// UTF-8-safe character windows: at most 2,048 UTF-8 bytes / 2,048 tokenizer tokens per chunk,
/// plus at most 512 bytes heading context. 64-character overlap; at most 1,024 chunks per note.
pub(super) fn chunks(markdown: &str) -> Result<Vec<String>, &'static str> {
    if markdown.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    let mut context = String::new();
    let mut buffer = String::new();
    for line in markdown.split_inclusive('\n') {
        if line.starts_with('#') {
            context = line.chars().take(128).collect();
        }
        for c in line.chars() {
            buffer.push(c);
            if buffer.chars().count() >= 512 {
                result.push(format!("{context}\n{buffer}"));
                if result.len() > 1024 {
                    return Err("chunk_limit");
                }
                buffer = buffer
                    .chars()
                    .rev()
                    .take(64)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
            }
        }
    }
    if !buffer.trim().is_empty() {
        result.push(format!("{context}\n{buffer}"));
    }
    if result.len() > 1024 {
        return Err("chunk_limit");
    }
    Ok(result)
}
#[cfg(test)]
#[path = "../../../tests/unit/domains/notebooks/markdown_tests.rs"]
mod tests;
