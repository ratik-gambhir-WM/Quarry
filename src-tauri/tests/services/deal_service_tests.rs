use super::*;
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "quarry-deal-service-{name}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write_file(&self, relative_path: &str, content: &[u8]) -> PathBuf {
        let path = self.path.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content).unwrap();
        path
    }

    fn create_dir(&self, relative_path: &str) -> PathBuf {
        let path = self.path.join(relative_path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn deal_input() -> SaveDealAndExtractInput {
    SaveDealAndExtractInput {
        deal_name: " Project Gamma ".to_string(),
        main_data_room_folder: " /data-room ".to_string(),
        deal_type: " Buy-side ".to_string(),
        pe_firm: " West Monroe Capital ".to_string(),
        target_company: Some(" Target Co ".to_string()),
        buyer_or_platform_company: Some(" Platform Co ".to_string()),
        parent_or_seller_company: Some(" ".to_string()),
        carve_out_business: None,
    }
}

fn deal_fixture() -> Deal {
    Deal {
        id: 7,
        deal_name: "Project Gamma".to_string(),
        main_data_room_folder: "/tmp/data-room".to_string(),
        deal_type: "Buy-side".to_string(),
        pe_firm: "West Monroe Capital".to_string(),
        status: "active".to_string(),
        target_company: Some("Target Co".to_string()),
        buyer_or_platform_company: Some("Platform Co".to_string()),
        parent_or_seller_company: None,
        carve_out_business: None,
        created_at: "2026-07-24T00:00:00Z".to_string(),
        updated_at: "2026-07-24T00:00:00Z".to_string(),
    }
}

fn selected_file_input(
    sow_file_path: impl Into<String>,
    project_timeline_file_path: impl Into<String>,
) -> ExtractDealQuestionsAndThesisInput {
    ExtractDealQuestionsAndThesisInput {
        deal_id: 7,
        sow_file_path: Some(sow_file_path.into()),
        project_timeline_file_path: Some(project_timeline_file_path.into()),
    }
}

fn matched_file(relative_path: &str, matched_on: Vec<String>) -> MatchedDealFile {
    MatchedDealFile {
        data_base64: Some("YWJjMTIz".to_string()),
        mime_type: Some("application/pdf"),
        source_file: DealSourceFile {
            path: format!("/tmp/{relative_path}"),
            filename: Path::new(relative_path)
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string(),
            relative_path: relative_path.to_string(),
            size_bytes: 123,
            matched_on,
            text_extracted: false,
            text_truncated: false,
        },
    }
}

#[test]
fn validate_deal_input_accepts_required_fields() {
    assert!(validate_deal_input(&deal_input()).is_ok());
}

#[test]
fn validate_deal_input_rejects_missing_required_fields() {
    let mut input = deal_input();
    input.deal_name = " ".to_string();
    assert_eq!(
        validate_deal_input(&input),
        Err("dealName is required".to_string())
    );

    let mut input = deal_input();
    input.main_data_room_folder = " ".to_string();
    assert_eq!(
        validate_deal_input(&input),
        Err("mainDataRoomFolder is required".to_string())
    );

    let mut input = deal_input();
    input.deal_type = " ".to_string();
    assert_eq!(
        validate_deal_input(&input),
        Err("dealType is required".to_string())
    );

    let mut input = deal_input();
    input.pe_firm = " ".to_string();
    assert_eq!(
        validate_deal_input(&input),
        Err("peFirm is required".to_string())
    );
}

#[test]
fn validate_deal_input_enforces_supported_types_and_type_specific_companies() {
    let mut input = deal_input();
    input.deal_type = "Unknown".to_string();
    assert_eq!(
        validate_deal_input(&input),
        Err("dealType is not supported".to_string())
    );

    let mut input = deal_input();
    input.target_company = None;
    assert_eq!(
        validate_deal_input(&input),
        Err("targetCompany is required for Buy-side deals".to_string())
    );

    let mut input = deal_input();
    input.buyer_or_platform_company = None;
    assert_eq!(
        validate_deal_input(&input),
        Err("buyerOrPlatformCompany is required for Buy-side deals".to_string())
    );

    let mut input = deal_input();
    input.deal_type = "Carve-out".to_string();
    input.target_company = None;
    input.buyer_or_platform_company = None;
    input.parent_or_seller_company = Some("Parent Co".to_string());
    input.carve_out_business = Some("Division".to_string());
    assert!(validate_deal_input(&input).is_ok());
}

#[test]
fn save_deal_with_repository_trims_input_and_uses_mock_repository() {
    let captured = RefCell::new(None);
    let deal = save_deal_with_repository(&deal_input(), |record| {
        captured.replace(Some((
            record.deal_name.to_string(),
            record.main_data_room_folder.to_string(),
            record.deal_type.to_string(),
            record.pe_firm.to_string(),
            record.target_company.map(str::to_string),
            record.buyer_or_platform_company.map(str::to_string),
            record.parent_or_seller_company.map(str::to_string),
            record.carve_out_business.map(str::to_string),
        )));
        Ok(deal_fixture())
    })
    .unwrap();

    assert_eq!(deal.id, 7);
    assert_eq!(
        captured.into_inner().unwrap(),
        (
            "Project Gamma".to_string(),
            "/data-room".to_string(),
            "Buy-side".to_string(),
            "West Monroe Capital".to_string(),
            Some("Target Co".to_string()),
            Some("Platform Co".to_string()),
            None,
            None,
        )
    );
}

#[test]
fn save_deal_with_repository_propagates_repository_errors() {
    let error = save_deal_with_repository(&deal_input(), |_| Err("insert failed".to_string()));

    assert_eq!(error.unwrap_err(), "insert failed");
}

#[test]
fn collect_sow_and_timeline_files_errors_for_missing_or_non_directory_roots() {
    let root = TestDir::new("bad-root");
    let file_path = root.write_file("file.txt", b"hello");

    let missing_error =
        match collect_sow_and_timeline_files_with_options(&root.path().join("missing"), true) {
            Err(error) => error,
            Ok(_) => panic!("expected missing root to return an error"),
        };
    assert!(missing_error.contains("does not exist"));

    let non_directory_error = match collect_sow_and_timeline_files_with_options(&file_path, true) {
        Err(error) => error,
        Ok(_) => panic!("expected file root to return an error"),
    };
    assert!(non_directory_error.contains("not a folder"));
}

#[test]
fn collect_sow_and_timeline_files_prefers_admin_matches() {
    let root = TestDir::new("admin-first");
    root.write_file(".01 Admin/Project Timeline.pdf", b"timeline");
    root.write_file("Commercial/SOW.pdf", b"sow");

    let files = collect_sow_and_timeline_files_with_options(root.path(), true).unwrap();

    assert_eq!(files.len(), 2);
    assert_eq!(
        files[0].source_file.relative_path,
        ".01 Admin/Project Timeline.pdf"
    );
    assert_eq!(files[0].source_file.matched_on, vec!["Project Timeline"]);
    assert_eq!(files[1].source_file.relative_path, "Commercial/SOW.pdf");
    assert_eq!(files[1].source_file.matched_on, vec!["SOW"]);
}

#[test]
fn collect_sow_and_timeline_files_uses_admin_matches_when_both_types_are_found() {
    let root = TestDir::new("admin-both");
    root.write_file(".01 Admin/Project Timeline.pdf", b"timeline");
    root.write_file(".01 Admin/SOW.pdf", b"sow");
    root.write_file("Commercial/SOW.pdf", b"sow");

    let files = collect_sow_and_timeline_files_with_options(root.path(), true).unwrap();

    assert_eq!(files.len(), 2);
    assert_eq!(
        files[0].source_file.relative_path,
        ".01 Admin/Project Timeline.pdf"
    );
    assert_eq!(files[1].source_file.relative_path, ".01 Admin/SOW.pdf");
}

#[test]
fn collect_sow_and_timeline_files_falls_back_to_root_when_admin_has_no_matches() {
    let root = TestDir::new("admin-fallback");
    root.write_file(".01 Admin/readme.txt", b"admin notes");
    root.write_file("Commercial/SOW.pdf", b"sow");

    let files = collect_sow_and_timeline_files_with_options(root.path(), true).unwrap();

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].source_file.relative_path, "Commercial/SOW.pdf");
    assert_eq!(files[0].source_file.matched_on, vec!["SOW"]);
}

#[test]
fn collect_matching_files_from_root_matches_file_names_only() {
    let root = TestDir::new("filename-only");
    root.write_file("SOW Folder/agenda.txt", b"agenda");
    root.write_file("Admin/Final SOW.txt", b"scope");
    root.write_file("Admin/Project Timeline.txt", b"timeline");
    root.write_file("Admin/empty SOW.txt", b"");

    let mut files = Vec::new();
    collect_matching_files_from_root_with_options(root.path(), root.path(), &mut files, true)
        .unwrap();
    files.sort_by(|left, right| {
        left.source_file
            .relative_path
            .cmp(&right.source_file.relative_path)
    });

    assert_eq!(files.len(), 2);
    assert_eq!(files[0].source_file.relative_path, "Admin/Final SOW.txt");
    assert_eq!(files[0].source_file.matched_on, vec!["SOW"]);
    assert_eq!(files[0].mime_type, Some("text/plain"));
    let encoded_scope = general_purpose::STANDARD.encode(b"scope");
    assert_eq!(
        files[0].data_base64.as_deref(),
        Some(encoded_scope.as_str())
    );
    assert_eq!(
        files[1].source_file.relative_path,
        "Admin/Project Timeline.txt"
    );
    assert_eq!(files[1].source_file.matched_on, vec!["Project Timeline"]);
}

#[test]
fn collect_matching_files_from_root_ignores_office_lock_files() {
    let root = TestDir::new("lock-files");
    root.write_file("Admin/~$ Final SOW.docx", b"lock");
    root.write_file("Admin/Final SOW.txt", b"scope");

    let mut files = Vec::new();
    collect_matching_files_from_root_with_options(root.path(), root.path(), &mut files, false)
        .unwrap();

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].source_file.relative_path, "Admin/Final SOW.txt");
}

#[test]
fn load_selected_deal_files_encodes_selected_files_and_rejects_outside_paths() {
    let root = TestDir::new("selected-files");
    let sow = root.write_file("Admin/Final SOW.txt", b"scope");
    let timeline = root.write_file("Admin/Project Timeline.txt", b"timeline");
    let outside_root = TestDir::new("selected-files-outside");
    let outside = outside_root.write_file("Project Timeline.txt", b"outside");
    let mut deal = deal_fixture();
    deal.main_data_room_folder = root.path().display().to_string();

    let files = load_selected_deal_files(
        &deal,
        &selected_file_input(sow.display().to_string(), timeline.display().to_string()),
    )
    .unwrap();

    assert_eq!(files.len(), 2);
    assert_eq!(files[0].source_file.relative_path, "Admin/Final SOW.txt");
    assert_eq!(files[0].source_file.matched_on, vec!["SOW"]);
    assert_eq!(
        files[0].data_base64.as_deref(),
        Some(general_purpose::STANDARD.encode(b"scope").as_str())
    );
    assert_eq!(
        files[1].source_file.relative_path,
        "Admin/Project Timeline.txt"
    );

    let outside_error = match load_selected_deal_files(
        &deal,
        &selected_file_input(sow.display().to_string(), outside.display().to_string()),
    ) {
        Err(error) => error,
        Ok(_) => panic!("expected outside selected file to return an error"),
    };
    assert!(outside_error.contains("outside the deal data room"));
}

#[test]
fn selected_deal_file_paths_ignores_blank_paths_and_dedupes_matches() {
    assert_eq!(
        selected_deal_file_paths(&selected_file_input(" /tmp/SOW.pdf ", " /tmp/SOW.pdf ")).unwrap(),
        vec!["/tmp/SOW.pdf"]
    );

    assert_eq!(
        selected_deal_file_paths(&selected_file_input(" ", "/tmp/timeline.pdf")).unwrap(),
        vec!["/tmp/timeline.pdf"]
    );

    assert!(
        selected_deal_file_paths(&ExtractDealQuestionsAndThesisInput {
            deal_id: 7,
            sow_file_path: None,
            project_timeline_file_path: None,
        })
        .unwrap()
        .is_empty()
    );
}

#[test]
fn measure_data_room_counts_non_ignored_files_and_bytes() {
    let root = TestDir::new("data-room-measure");
    root.write_file("Admin/SOW.txt", b"scope");
    root.write_file("Commercial/Timeline.txt", b"timeline");
    root.write_file("Admin/~$ Draft SOW.docx", b"lock");

    let (document_count, data_room_size_bytes) = measure_data_room(root.path()).unwrap();

    assert_eq!(document_count, 2);
    assert_eq!(data_room_size_bytes, 13);
}

#[test]
fn admin_search_roots_finds_admin_like_folders_within_depth_limit() {
    let root = TestDir::new("admin-roots");
    let admin = root.create_dir(".01 Admin");
    let nested_admin = root.create_dir("A/B/Administration");
    root.create_dir("A/B/C/Admin");
    root.create_dir("Commercial");

    let roots = admin_search_roots(root.path());

    assert_eq!(roots, vec![admin, nested_admin]);
}

#[test]
fn is_admin_folder_name_accepts_numbered_and_delimited_admin_names() {
    assert!(is_admin_folder_name(".01 Admin"));
    assert!(is_admin_folder_name("02_Administration"));
    assert!(is_admin_folder_name("03-admin"));
    assert!(!is_admin_folder_name("Commercial"));
}

#[test]
fn extract_deal_questions_and_thesis_returns_empty_without_files() {
    let extraction = tokio_test_block_on(extract_deal_questions_and_thesis_from_files(
        &deal_fixture(),
        &[],
    ))
    .unwrap();

    assert!(extraction.key_questions.is_empty());
    assert_eq!(extraction.investment_thesis, "");
}

#[test]
fn extract_deal_questions_and_thesis_returns_empty_without_attachable_files() {
    let files = [MatchedDealFile {
        source_file: DealSourceFile {
            path: "/tmp/Final SOW.unsupported".to_string(),
            filename: "Final SOW.unsupported".to_string(),
            relative_path: "Final SOW.unsupported".to_string(),
            size_bytes: 10,
            matched_on: vec!["SOW".to_string()],
            text_extracted: false,
            text_truncated: false,
        },
        data_base64: None,
        mime_type: None,
    }];

    let extraction = tokio_test_block_on(extract_deal_questions_and_thesis_from_files(
        &deal_fixture(),
        &files,
    ))
    .unwrap();

    assert!(extraction.key_questions.is_empty());
    assert_eq!(extraction.investment_thesis, "");
}

#[test]
fn build_deal_extraction_prompt_includes_metadata_manifest_and_date_rule() {
    let files = [
        matched_file("Admin/SOW v1.pdf", vec!["SOW".to_string()]),
        matched_file(
            "Admin/Project Timeline v2.pdf",
            vec!["Project Timeline".to_string()],
        ),
    ];
    let file_refs = files.iter().collect::<Vec<_>>();

    let prompt = build_deal_extraction_prompt(&deal_fixture(), &file_refs);

    assert!(prompt.contains("Deal name: Project Gamma"));
    assert!(prompt.contains("Target company: Target Co"));
    assert!(prompt.contains("Admin/SOW v1.pdf"));
    assert!(prompt.contains("Admin/Project Timeline v2.pdf"));
    assert!(prompt.contains("explicitly labeled as key questions"));
    assert!(prompt.contains("Do not create, infer, rewrite, synthesize, or add any key questions"));
    assert!(prompt.contains("Always return investmentThesis as an empty string"));
    assert!(prompt.contains("\"keyQuestions\""));
    assert!(prompt.contains("\"investmentThesis\""));
}

#[test]
fn build_deal_extraction_prompt_asks_for_equity_story_on_sell_side() {
    let files = [matched_file("Admin/SOW v1.pdf", vec!["SOW".to_string()])];
    let file_refs = files.iter().collect::<Vec<_>>();
    let mut deal = deal_fixture();
    deal.deal_type = "Sell-side".to_string();

    let prompt = build_deal_extraction_prompt(&deal, &file_refs);

    assert!(prompt.contains("investmentThesis must contain a concise equity story for Target Co"));
    assert!(prompt.contains("buyer-facing value proposition"));
    assert!(!prompt.contains("Always return investmentThesis as an empty string"));
}

#[test]
fn parse_deal_extraction_parses_raw_and_fenced_json() {
    let raw = parse_deal_extraction(
        r#"{"keyQuestions":["What is the implementation risk?"],"investmentThesis":"Strong target."}"#,
    )
    .unwrap();
    assert_eq!(raw.key_questions, vec!["What is the implementation risk?"]);
    assert_eq!(raw.investment_thesis, "Strong target.");

    let fenced = parse_deal_extraction(
        "```json\n{\"keyQuestions\":[\"What changed?\"],\"investmentThesis\":\"Updated case.\"}\n```",
    )
    .unwrap();
    assert_eq!(fenced.key_questions, vec!["What changed?"]);
    assert_eq!(fenced.investment_thesis, "Updated case.");
}

#[test]
fn parse_deal_extraction_errors_for_invalid_json() {
    let error = parse_deal_extraction("not json").unwrap_err();

    assert!(error.contains("failed to parse deal extraction JSON"));
}

#[test]
fn encode_supported_file_base64_encodes_supported_files_and_skips_unsupported_files() {
    let root = TestDir::new("encode");
    let supported = root.write_file("SOW.txt", b"scope");
    let unsupported = root.write_file("SOW.bin", b"scope");

    assert_eq!(
        encode_supported_file(&supported).unwrap().unwrap(),
        general_purpose::STANDARD.encode(b"scope")
    );
    assert!(encode_supported_file(&unsupported).is_none());
}

#[test]
fn matching_terms_detects_case_insensitive_sow_and_project_timeline() {
    assert_eq!(matching_terms("final SOW.pdf"), vec!["SOW"]);
    assert_eq!(matching_terms("scope of work.docx"), vec!["SOW"]);
    assert_eq!(
        matching_terms("PROJECT TIMELINE v2.xlsx"),
        vec!["Project Timeline"]
    );
    assert_eq!(
        matching_terms("Diligence Schedule.xlsx"),
        vec!["Project Timeline"]
    );
    assert_eq!(
        matching_terms("Project Plan.xlsx"),
        vec!["Project Timeline"]
    );
    assert_eq!(matching_terms("Workplan.pdf"), vec!["Project Timeline"]);
    assert_eq!(
        matching_terms("SOW - Project Timeline.pdf"),
        vec!["SOW", "Project Timeline"]
    );
    assert!(matching_terms("Commercial model.pdf").is_empty());
}

#[test]
fn trim_optional_trims_values_and_removes_blanks() {
    assert_eq!(trim_optional(Some("  Target Co  ")), Some("Target Co"));
    assert_eq!(trim_optional(Some("   ")), None);
    assert_eq!(trim_optional(None), None);
}

fn tokio_test_block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}
