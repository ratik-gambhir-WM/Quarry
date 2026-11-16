//! Shared version-one English and technical-token analysis.
use super::ids::sha256_hex;
/// Version-one analyzer: English body terms plus exact technical-token identities.
/// Hashing makes technical tokens bounded ordinary lexemes, with no tsquery syntax.
pub(crate) fn lexical_parts(text: &str) -> (String, String) {
    static TOKENS: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"[\p{L}\p{N}\p{M}_+#.]+").expect("static document token pattern")
    });
    let mut body = Vec::new();
    let mut technical = Vec::new();
    for token in TOKENS.find_iter(text) {
        let token = token.as_str().trim_end_matches('.');
        if !token.chars().any(char::is_alphanumeric) {
            continue;
        }
        if token.contains(['+', '#', '_', '.']) {
            technical.push(format!(
                "quarrytech{}",
                sha256_hex(token.to_lowercase().as_bytes())
            ));
        } else {
            body.push(token.to_owned());
        }
    }
    (body.join(" "), technical.join(" "))
}
