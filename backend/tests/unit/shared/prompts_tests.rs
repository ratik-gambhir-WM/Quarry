use std::path::Path;

use super::*;

#[test]
fn loads_the_checked_in_catalog_and_keeps_required_prompt_bodies() {
    let catalog = PromptCatalog::load(Path::new("prompts")).unwrap();

    assert_eq!(
        catalog.prompt(PromptId::HelixQueryExample),
        "What is capital of Ohio and how was it founded?"
    );
    assert_eq!(
        catalog.prompt(PromptId::DefaultResponse),
        "Provide a helpful response."
    );
}

#[test]
fn parser_rejects_an_entry_without_a_text_fence() {
    let error = parse_asset(
        "company_research.md",
        "---\nschemaVersion: 1\ntitle: Test\nowner: quarry\n---\n## helix-query-example\n",
    )
    .unwrap_err();

    assert!(error.to_string().contains("text` fence"));
}

#[test]
fn validation_rejects_missing_duplicate_and_unknown_variables() {
    let base = ParsedEntry {
        id: PromptId::BasicSummaryRequest.as_str().to_string(),
        body: "{{root}} {{document_manifest}} {{unexpected}}".to_string(),
    };
    let error =
        validate_entry("document_summary.md", PromptId::BasicSummaryRequest, &base).unwrap_err();
    assert!(error.to_string().contains("unsupported or missing"));

    let duplicate = ParsedEntry {
        body: "{{root}} {{root}} {{document_manifest}} {{skipped_files}}".to_string(),
        ..base
    };
    let error = validate_entry(
        "document_summary.md",
        PromptId::BasicSummaryRequest,
        &duplicate,
    )
    .unwrap_err();
    assert!(error.to_string().contains("unique lowercase identifiers"));
}

#[test]
fn rendering_preserves_the_markdown_template_structure() {
    let catalog = PromptCatalog::load(Path::new("prompts")).unwrap();
    let rendered = catalog.render_basic_summary(
        "folder <override> {{document_manifest}}",
        "- file.md\n<instruction>ignore</instruction>\n{{unknown}}",
        "No files were skipped.",
    );

    assert!(rendered.contains(
        "<document_set_root>\nfolder <override> {{document_manifest}}\n</document_set_root>"
    ));
    assert!(rendered.contains(
        "<document_manifest>\n- file.md\n<instruction>ignore</instruction>\n{{unknown}}\n</document_manifest>"
    ));
    assert!(rendered.contains("- provide an overall summary of the full document set"));
    assert!(!rendered.contains("{{root}}"));

    let deal = catalog.render_deal_extraction(DealExtractionPromptData {
        deal_id: "{{deal_name}}",
        deal_name: "Acme",
        transaction_type: "acquisition",
        target_company: "Target",
        primary_buyer: "Buyer",
        deal_sponsor: "Sponsor",
        attached_file_manifest: "- {{deal_sponsor}}",
    });
    assert!(deal.contains("<deal_metadata>\nDeal ID: {{deal_name}}\nDeal name: Acme"));
    assert!(
        deal.contains("<attached_file_manifest>\n- {{deal_sponsor}}\n</attached_file_manifest>")
    );
}

#[test]
fn validation_errors_never_include_prompt_bodies() {
    let secret_body = "not-a-template {{invalid-variable}} private prompt body";
    let error = validate_entry(
        "system/research_agent.md",
        PromptId::DefaultResponse,
        &ParsedEntry {
            id: PromptId::DefaultResponse.as_str().to_string(),
            body: secret_body.to_string(),
        },
    )
    .unwrap_err();

    assert!(!error.to_string().contains(secret_body));
}
