use super::*;
use std::{
    env,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn build_file_input_item_uses_input_file_type_for_file_id() {
    let item = build_input_item(&ResponsesFileInput::FileId("file-123")).unwrap();

    assert_eq!(
        item,
        json!({
            "type": "input_file",
            "file_id": "file-123",
        })
    );
}

#[test]
fn build_file_input_item_wraps_base64_payload_in_data_url() {
    let item = build_input_item(&ResponsesFileInput::FileData {
        filename: "draconomicon.pdf",
        mime_type: "application/pdf",
        data_base64: "YWJjMTIz",
    })
    .unwrap();

    assert_eq!(
        item,
        json!({
            "type": "input_file",
            "filename": "draconomicon.pdf",
            "file_data": "data:application/pdf;base64,YWJjMTIz",
        })
    );
}

#[test]
fn build_responses_request_body_places_files_and_prompt_in_user_content() {
    let request_body = build_responses_request_body(
        "gpt-5",
        "You are a helpful assistant.",
        "What is the first dragon in the book?",
        Some(&[ResponsesFileInput::FileData {
            filename: "draconomicon.pdf",
            mime_type: "application/pdf",
            data_base64: "YWJjMTIz",
        }]),
    )
    .unwrap();

    assert_eq!(
        request_body,
        json!({
            "model": "gpt-5",
            "instructions": "You are a helpful assistant.",
            "input": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "input_file",
                            "filename": "draconomicon.pdf",
                            "file_data": "data:application/pdf;base64,YWJjMTIz",
                        },
                        {
                            "type": "input_text",
                            "text": "What is the first dragon in the book?",
                        }
                    ]
                }
            ]
        })
    );
}

#[test]
fn build_input_item_wraps_image_payload_as_input_image() {
    let item = build_input_item(&ResponsesFileInput::ImageData {
        mime_type: "image/png",
        data_base64: "YWJjMTIz",
        detail: Some("high"),
    })
    .unwrap();

    assert_eq!(
        item,
        json!({
            "type": "input_image",
            "image_url": "data:image/png;base64,YWJjMTIz",
            "detail": "high",
        })
    );
}

#[test]
fn build_file_input_item_reads_file_path_as_input_file_data_url() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let file_path = env::temp_dir().join(format!("openai-client-test-{unique}.pdf"));
    fs::write(&file_path, b"abc123").unwrap();

    let item = build_input_item(&ResponsesFileInput::FilePath(&file_path)).unwrap();

    assert_eq!(
        item,
        json!({
            "type": "input_file",
            "filename": file_path.file_name().and_then(|name| name.to_str()).unwrap(),
            "file_data": "data:application/pdf;base64,YWJjMTIz",
        })
    );

    fs::remove_file(&file_path).unwrap();
}

#[test]
fn infer_file_mime_type_returns_pdf_for_pdf_extension() {
    assert_eq!(
        infer_file_mime_type(Path::new("draconomicon.pdf")),
        "application/pdf"
    );
}

#[test]
fn infer_file_mime_type_falls_back_to_octet_stream_for_unknown_extension() {
    assert_eq!(
        infer_file_mime_type(Path::new("archive.unknown")),
        "application/octet-stream"
    );
}

#[test]
fn openai_error_reason_extracts_api_error_message() {
    let reason = openai_error_reason(
        reqwest::StatusCode::TOO_MANY_REQUESTS,
        r#"{"error":{"message":"Embedding rate limit exceeded","type":"rate_limit_error"}}"#,
    );

    assert_eq!(reason, "Embedding rate limit exceeded");
}

#[test]
fn openai_error_reason_falls_back_to_http_reason_for_empty_body() {
    let reason = openai_error_reason(reqwest::StatusCode::BAD_GATEWAY, "  ");

    assert_eq!(reason, "Bad Gateway");
}

#[test]
fn openai_error_reason_limits_unstructured_response_body() {
    let response_body = "x".repeat(MAX_LOGGED_ERROR_REASON_CHARS + 1);
    let reason = openai_error_reason(reqwest::StatusCode::BAD_GATEWAY, &response_body);

    assert_eq!(reason.chars().count(), MAX_LOGGED_ERROR_REASON_CHARS + 1);
    assert!(reason.ends_with('…'));
}

#[test]
fn extract_response_text_prefers_output_text_field() {
    let response_json = json!({
        "output_text": "  First dragon: Aasterinian  "
    });

    assert_eq!(
        extract_response_text(&response_json),
        Some("First dragon: Aasterinian".to_string())
    );
}

#[test]
fn extract_response_text_collects_text_parts_from_output_content() {
    let response_json = json!({
        "output": [
            {
                "content": [
                    {
                        "type": "output_text",
                        "text": "First paragraph"
                    },
                    {
                        "type": "text",
                        "text": "Second paragraph"
                    }
                ]
            }
        ]
    });

    assert_eq!(
        extract_response_text(&response_json),
        Some("First paragraph\nSecond paragraph".to_string())
    );
}

#[test]
fn extract_response_text_returns_none_when_no_text_is_present() {
    let response_json = json!({
        "output": [
            {
                "content": [
                    {
                        "type": "input_file",
                        "filename": "draconomicon.pdf"
                    }
                ]
            }
        ]
    });

    assert_eq!(extract_response_text(&response_json), None);
}

#[test]
fn process_sse_event_collects_output_text_delta() {
    let raw_event = r#"event: response.output_text.delta
data: {"type":"response.output_text.delta","delta":"Hello"}
"#;
    let mut streamed_text = String::new();
    let mut completed_text = None;
    let mut deltas = Vec::new();

    process_sse_event(
        raw_event,
        &mut streamed_text,
        &mut completed_text,
        &mut |delta| deltas.push(delta.to_string()),
    )
    .unwrap();

    assert_eq!(streamed_text, "Hello");
    assert_eq!(deltas, vec!["Hello"]);
    assert_eq!(completed_text, None);
}

#[test]
fn process_sse_events_handles_crlf_boundaries() {
    let mut pending =
            "event: response.output_text.delta\r\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"Hi\"}\r\n\r\n"
                .to_string();
    let mut streamed_text = String::new();
    let mut completed_text = None;
    let mut deltas = Vec::new();

    process_sse_events(
        &mut pending,
        &mut streamed_text,
        &mut completed_text,
        &mut |delta| deltas.push(delta.to_string()),
    )
    .unwrap();

    assert!(pending.is_empty());
    assert_eq!(streamed_text, "Hi");
    assert_eq!(deltas, vec!["Hi"]);
    assert_eq!(completed_text, None);
}
