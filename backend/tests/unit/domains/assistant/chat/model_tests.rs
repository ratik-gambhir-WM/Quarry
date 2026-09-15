use super::*;

#[test]
fn context_requires_complete_alternating_pairs_and_preserves_text() {
    let value =
        r#"[{"role":"user","content":"  question  "},{"role":"assistant","content":"answer"}]"#;
    let context = parse_context(value).unwrap();
    assert_eq!(context[0].content, "  question  ");

    assert!(parse_context(r#"[{"role":"assistant","content":"answer"}]"#).is_err());
    assert!(parse_context(r#"[{"role":"user","content":"question"}]"#).is_err());
    assert!(parse_context(
        r#"[{"role":"user","content":"q","extra":true},{"role":"assistant","content":"a"}]"#
    )
    .is_err());
}

#[test]
fn scalar_validation_preserves_text_but_normalizes_model() {
    assert_eq!(
        validate_prompt("  prompt  ".to_string()).unwrap(),
        "  prompt  "
    );
    assert_eq!(
        validate_model(Some("  gpt-5.5  ".to_string()))
            .unwrap()
            .as_deref(),
        Some("gpt-5.5")
    );
    assert!(validate_model(Some(" \n ".to_string())).is_err());
}
