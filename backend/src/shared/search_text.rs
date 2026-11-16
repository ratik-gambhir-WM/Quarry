//! Shared version-one English and technical-token analysis.
use super::ids::sha256_hex;
/// Version-one analyzer: English body terms plus exact technical-token identities.
/// Hashing makes technical tokens bounded ordinary lexemes, with no tsquery syntax.
fn tokens(text: &str) -> impl Iterator<Item = &str> {
    static TOKENS: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"[\p{L}\p{N}\p{M}_+#.]+").expect("static document token pattern")
    });
    TOKENS.find_iter(text).filter_map(|token| {
        let token = token.as_str().trim_end_matches('.');
        token.chars().any(char::is_alphanumeric).then_some(token)
    })
}

fn technical_lexeme(token: &str) -> Option<String> {
    token
        .contains(['+', '#', '_', '.'])
        .then(|| format!("quarrytech{}", sha256_hex(token.to_lowercase().as_bytes())))
}

pub(crate) fn lexical_parts(text: &str) -> (String, String) {
    let mut body = Vec::new();
    let mut technical = Vec::new();
    for token in tokens(text) {
        if let Some(lexeme) = technical_lexeme(token) {
            technical.push(lexeme);
        } else {
            body.push(token.to_owned());
        }
    }
    (body.join(" "), technical.join(" "))
}

/// Keep technical identities in the original word stream so SQL phrase positions remain valid.
/// English text search still owns ordinary-word stemming and stop-word gaps.
pub(crate) fn lexical_phrase(text: &str) -> String {
    tokens(text)
        .map(|token| technical_lexeme(token).unwrap_or_else(|| token.to_owned()))
        .collect::<Vec<_>>()
        .join(" ")
}
