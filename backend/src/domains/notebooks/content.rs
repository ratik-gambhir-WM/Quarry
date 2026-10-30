//! Validation for the installed BlockNote 0.56 native document schema. Never normalizes body text.
use serde_json::{Map, Value};
use std::collections::HashSet;
type Validation = Result<(), &'static str>;

pub(super) fn validate(blocks: &Value) -> Validation {
    if serde_json::to_vec(blocks)
        .map_err(|_| "invalid blocks")?
        .len()
        > 1024 * 1024
    {
        return Err("note content exceeds size limit");
    }
    let mut nodes = 0;
    let mut pending = vec![(blocks, 0)];
    while let Some((value, depth)) = pending.pop() {
        nodes += 1;
        if nodes > 10_000 || depth > 110 {
            return Err("note content exceeds structure limit");
        }
        match value {
            Value::Array(items) => pending.extend(items.iter().map(|v| (v, depth + 1))),
            Value::Object(items) => pending.extend(items.values().map(|v| (v, depth + 1))),
            _ => {}
        }
    }
    let blocks = blocks.as_array().ok_or("blocks must be an array")?;
    let mut ids = HashSet::new();
    validate_blocks(blocks, 1, &mut ids)
}
fn object<'a>(value: &'a Value, allowed: &[&str]) -> Result<&'a Map<String, Value>, &'static str> {
    let map = value.as_object().ok_or("invalid content object")?;
    if map.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("unsupported content field");
    }
    Ok(map)
}
fn validate_blocks(blocks: &[Value], depth: usize, ids: &mut HashSet<String>) -> Validation {
    if depth > 32 {
        return Err("note content exceeds nesting limit");
    }
    for block in blocks {
        let map = object(block, &["id", "type", "props", "content", "children"])?;
        let id = map
            .get("id")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty() && s.len() <= 200)
            .ok_or("invalid block id")?;
        if !ids.insert(id.to_owned()) {
            return Err("duplicate block id");
        }
        let kind = block["type"].as_str().ok_or("invalid block type")?;
        if ![
            "paragraph",
            "heading",
            "bulletListItem",
            "numberedListItem",
            "checkListItem",
            "quote",
            "codeBlock",
            "table",
        ]
        .contains(&kind)
        {
            return Err("unsupported block type");
        }
        props(&block["props"], kind)?;
        if kind == "table" {
            table(&block["content"])?;
        } else if kind == "codeBlock" {
            // BlockNote plain content is an array of unstyled text inline nodes.
            inline(&block["content"], false)?;
        } else {
            inline(&block["content"], true)?;
        }
        let children = block["children"]
            .as_array()
            .ok_or("invalid block children")?;
        validate_blocks(children, depth + 1, ids)?;
    }
    Ok(())
}
fn props(value: &Value, kind: &str) -> Validation {
    let map = value.as_object().ok_or("invalid block props")?;
    for (key, value) in map {
        let valid = match key.as_str() {
            "textColor" | "backgroundColor" if kind != "codeBlock" => value
                .as_str()
                .is_some_and(|s| s.len() <= 200 && !s.chars().any(char::is_control)),
            "textAlignment" if kind != "codeBlock" => value
                .as_str()
                .is_some_and(|s| ["left", "center", "right", "justify"].contains(&s)),
            "level" if kind == "heading" => value.as_u64().is_some_and(|v| (1..=3).contains(&v)),
            "isToggleable" if kind == "heading" => value.is_boolean(),
            "checked" if kind == "checkListItem" => value.is_boolean(),
            "start" if kind == "numberedListItem" => {
                value.is_null()
                    || value
                        .as_i64()
                        .is_some_and(|v| (1..=i64::from(i32::MAX)).contains(&v))
            }
            "language" if kind == "codeBlock" => value
                .as_str()
                .is_some_and(|s| s.len() <= 100 && !s.chars().any(char::is_control)),
            "colspan" | "rowspan" if kind == "tableCell" => {
                value.as_u64().is_some_and(|v| (1..=1000).contains(&v))
            }
            _ => false,
        };
        if !valid {
            return Err("unsupported block property");
        }
    }
    Ok(())
}
fn inline(value: &Value, links: bool) -> Validation {
    for item in value.as_array().ok_or("invalid inline content")? {
        match item["type"].as_str() {
            Some("text") => {
                object(item, &["type", "text", "styles"])?;
                if !item["text"].is_string() {
                    return Err("invalid inline text");
                }
                let styles = item["styles"].as_object().ok_or("invalid text styles")?;
                for (key, value) in styles {
                    let valid = match key.as_str() {
                        "bold" | "italic" | "underline" | "strike" | "code" => value.is_boolean(),
                        "textColor" | "backgroundColor" => value
                            .as_str()
                            .is_some_and(|s| s.len() <= 200 && !s.chars().any(char::is_control)),
                        _ => false,
                    };
                    if !valid || (!links && !styles.is_empty()) {
                        return Err("unsupported text style");
                    }
                }
            }
            Some("link") if links => {
                object(item, &["type", "href", "content"])?;
                let href = item["href"].as_str().ok_or("invalid link")?;
                let url = url::Url::parse(href).map_err(|_| "invalid link")?;
                if href.len() > 2048
                    || href.chars().any(char::is_control)
                    || !["http", "https", "mailto"].contains(&url.scheme())
                    || !url.username().is_empty()
                    || url.password().is_some()
                {
                    return Err("unsafe link");
                }
                for text in item["content"].as_array().ok_or("invalid link content")? {
                    if text["type"] != "text" {
                        return Err("invalid link content");
                    }
                }
                inline(&item["content"], true)?;
            }
            _ => return Err("unsupported inline content"),
        }
    }
    Ok(())
}
fn table(value: &Value) -> Validation {
    object(
        value,
        &["type", "columnWidths", "headerRows", "headerCols", "rows"],
    )?;
    if value["type"] != "tableContent" {
        return Err("invalid table content");
    }
    let widths = value["columnWidths"]
        .as_array()
        .ok_or("invalid table widths")?;
    if widths.is_empty()
        || widths.len() > 1000
        || widths.iter().any(|v| {
            !v.is_null()
                && !v
                    .as_f64()
                    .is_some_and(|n| n.is_finite() && n > 0.0 && n <= 10000.0)
        })
    {
        return Err("invalid table widths");
    }
    let rows = value["rows"]
        .as_array()
        .filter(|r| !r.is_empty())
        .ok_or("invalid table rows")?;
    let mut occupied_until = vec![0; widths.len()];
    for (row_index, row) in rows.iter().enumerate() {
        object(row, &["cells"])?;
        let cells = row["cells"].as_array().ok_or("invalid table cells")?;
        let mut column = 0;
        for cell in cells {
            while column < widths.len() && occupied_until[column] > row_index {
                column += 1;
            }
            let colspan = usize::try_from(cell["props"]["colspan"].as_u64().unwrap_or(1))
                .map_err(|_| "invalid table span")?;
            let rowspan = usize::try_from(cell["props"]["rowspan"].as_u64().unwrap_or(1))
                .map_err(|_| "invalid table span")?;
            let end = column
                .checked_add(colspan)
                .filter(|end| *end <= widths.len())
                .ok_or("invalid table span")?;
            let until = row_index
                .checked_add(rowspan)
                .filter(|end| *end <= rows.len())
                .ok_or("invalid table span")?;
            if colspan == 0
                || rowspan == 0
                || occupied_until[column..end]
                    .iter()
                    .any(|end| *end > row_index)
            {
                return Err("overlapping table spans");
            }
            occupied_until[column..end].fill(until);
            column = end;
            if cell.is_array() {
                inline(cell, true)?;
            } else {
                object(cell, &["type", "props", "content"])?;
                if cell["type"] != "tableCell" {
                    return Err("invalid table cell");
                }
                props(&cell["props"], "tableCell")?;
                inline(&cell["content"], true)?;
            }
        }
        if occupied_until.iter().any(|until| *until <= row_index) {
            return Err("incomplete table row");
        }
    }
    for (key, max) in [("headerRows", rows.len()), ("headerCols", widths.len())] {
        if let Some(v) = value.get(key) {
            if !v
                .as_u64()
                .is_some_and(|n| usize::try_from(n).is_ok_and(|n| n <= max))
            {
                return Err("invalid table headers");
            }
        }
    }
    Ok(())
}
#[cfg(test)]
#[path = "../../../tests/unit/domains/notebooks/content_tests.rs"]
mod tests;
