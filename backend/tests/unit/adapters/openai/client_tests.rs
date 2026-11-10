use super::*;
use rig::{
    providers::openai::wire::Embeddings,
    wire::{Body, Encoded, Mode, Wire},
};
use std::{
    env,
    num::NonZeroUsize,
    time::{SystemTime, UNIX_EPOCH},
};

struct ResponsesServer {
    address: std::net::SocketAddr,
    task: tokio::task::JoinHandle<()>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl ResponsesServer {
    async fn start(router: axum::Router) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (shutdown, stopped) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
                .unwrap()
        });
        Self {
            address,
            task,
            shutdown: Some(shutdown),
        }
    }

    fn client(&self) -> OpenAiClient {
        OpenAiClient::with_responses_url(
            "unused-test-key",
            format!("http://{}/responses", self.address),
            crate::integration_support::prompt_catalog(),
        )
    }

    async fn stop(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        tokio::time::timeout(std::time::Duration::from_secs(2), &mut self.task)
            .await
            .expect("mock server shuts down")
            .expect("mock server exits cleanly");
    }
}

impl Drop for ResponsesServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn chat_from_stream(
    chunks: Vec<bytes::Bytes>,
) -> (Result<String, ChatStreamError>, Vec<String>) {
    use axum::{body::Body, routing::post, Router};
    let server = ResponsesServer::start(Router::new().route(
        "/responses",
        post(move || {
            let chunks = chunks.clone();
            async move {
                axum::response::Response::builder()
                    .header("content-type", "text/event-stream")
                    .body(Body::from_stream(futures_util::stream::iter(
                        chunks.into_iter().map(Ok::<_, std::io::Error>),
                    )))
                    .unwrap()
            }
        }),
    ))
    .await;
    let client = server.client();
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
    let generation = client.gen_chat_response_streaming(
        &[],
        "question",
        "instructions",
        "custom-model",
        &[],
        sender,
    );
    let (result, deltas) = tokio::join!(generation, async move {
        let mut deltas = Vec::new();
        while let Some(delta) = receiver.recv().await {
            deltas.push(delta);
        }
        deltas
    });
    server.stop().await;
    (result, deltas)
}

#[tokio::test]
async fn rig_stream_preserves_utf8_deltas_and_authoritative_completion() {
    let bytes = concat!(
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"hé\"}\r\n\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"output_text\":\"authoritative\"}}\r\n\r\n"
    ).as_bytes();
    let chunks = bytes.chunks(1).map(bytes::Bytes::copy_from_slice).collect();
    let (result, deltas) = chat_from_stream(chunks).await;
    assert_eq!(result.unwrap(), "authoritative");
    assert_eq!(deltas, ["hé"]);
}

#[tokio::test]
async fn rig_stream_rejects_failures_and_truncation() {
    for event in [
        "data: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\"}}\n\n".to_owned(),
        "data: {\"type\":\"response.incomplete\",\"response\":{\"status\":\"incomplete\",\"incomplete_details\":{\"reason\":\"max_output_tokens\"}}}\n\n".to_owned(),
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\n".to_owned(),
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"unterminated\"}".to_owned(),
    ] {
        let (result, _) = chat_from_stream(vec![bytes::Bytes::from(event)]).await;
        assert!(result.is_err());
    }
}

#[tokio::test]
async fn rig_sends_office_files_unchanged_in_buffered_streamed_and_chat_requests() {
    use axum::{routing::post, Json, Router};
    let requests = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let captured = requests.clone();
    let server = ResponsesServer::start(Router::new().route("/responses", post(move |Json(body): Json<Value>| {
        let captured = captured.clone();
        async move {
            let streaming = body["stream"] == true;
            captured.lock().await.push(body);
            let response = if streaming {
                "data: {\"type\":\"response.output_text.delta\",\"delta\":\"answer\"}\n\ndata: {\"type\":\"response.completed\",\"response\":{\"output_text\":\"answer\"}}\n\n".to_owned()
            } else { json!({"object": "response", "output_text": "answer"}).to_string() };
            axum::response::Response::builder().header("content-type", if streaming { "text/event-stream" } else { "application/json" })
                .body(axum::body::Body::from(response)).unwrap()
        }
    }))).await;
    let formats = [
        ("Original report.doc", "application/msword"),
        (
            "Original report.docx",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        ),
        (
            "Original deck.pptx",
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        ),
        (
            "Original model.xlsx",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
    ];
    let original_bytes = b"PK\x03\x04\x00\xff\x80\r\nOffice package bytes";
    let data = general_purpose::STANDARD.encode(original_bytes);
    let files = formats
        .iter()
        .map(|(name, mime)| ResponsesFileInput::FileData {
            filename: name,
            mime_type: mime,
            data_base64: &data,
        })
        .collect::<Vec<_>>();
    let client = server.client();
    assert_eq!(
        client
            .gen_model_response_with_files(
                Some("question"),
                Some("instructions"),
                Some("custom-model"),
                Some(&files)
            )
            .await
            .unwrap(),
        "answer"
    );
    assert_eq!(
        client
            .gen_model_response_with_files_streaming(
                Some("question"),
                Some("instructions"),
                Some("custom-model"),
                Some(&files),
                |_| {}
            )
            .await
            .unwrap(),
        "answer"
    );
    let history = [
        ChatMessageInput {
            role: ChatRole::User,
            content: "previous".into(),
        },
        ChatMessageInput {
            role: ChatRole::Assistant,
            content: "prior answer".into(),
        },
    ];
    let (sender, mut receiver) = tokio::sync::mpsc::channel(4);
    assert_eq!(
        client
            .gen_chat_response_streaming(
                &history,
                "question",
                "instructions",
                "custom-model",
                &files,
                sender
            )
            .await
            .unwrap(),
        "answer"
    );
    assert_eq!(receiver.recv().await.as_deref(), Some("answer"));
    let requests = requests.lock().await;
    assert_eq!(requests.len(), 3);
    for (index, body) in requests.iter().enumerate() {
        assert_eq!(body["instructions"], "instructions");
        let input = body["input"].as_array().unwrap();
        assert_eq!(input.len(), if index == 2 { 3 } else { 1 });
        if index == 2 {
            assert_eq!(input[0]["content"][0]["text"], "previous");
            assert_eq!(input[1]["content"][0]["text"], "prior answer");
            assert_eq!(body["store"], false);
        }
        let content = input.last().unwrap()["content"].as_array().unwrap();
        assert_eq!(content.len(), 5);
        for (part, (name, mime)) in content.iter().zip(formats) {
            assert_eq!(part["type"], "input_file");
            assert_eq!(part["filename"], name);
            assert_eq!(part["file_data"], format!("data:{mime};base64,{data}"));
            let (_, encoded) = part["file_data"].as_str().unwrap().split_once(',').unwrap();
            assert_eq!(
                general_purpose::STANDARD.decode(encoded).unwrap(),
                original_bytes
            );
        }
        assert_eq!(
            content[4],
            json!({"type": "input_text", "text": "question"})
        );
    }
    drop(requests);
    server.stop().await;
}

#[tokio::test]
async fn rig_embedder_preserves_order_and_rejects_invalid_provider_vectors() {
    use axum::{routing::post, Json, Router};
    let valid = json!({"data": [{"index":1,"embedding":[0.6,0.8]},{"index":0,"embedding":[1.0,0.0]}], "usage": {"prompt_tokens":2,"total_tokens":2}});
    let invalid = [
        json!({"data":[{"index":0,"embedding":[1.0,0.0]},{"index":0,"embedding":[0.6,0.8]}]}),
        json!({"data":[{"index":2,"embedding":[1.0,0.0]},{"index":1,"embedding":[0.6,0.8]}]}),
        json!({"data":[{"index":0,"embedding":[1.0]}]}),
        json!({"data":[{"index":0,"embedding":[1.0,0.0]}]}),
        json!({"data":[{"index":0,"embedding":[f64::MAX,0.0]},{"index":1,"embedding":[0.6,0.8]}]}),
    ];
    for (index, mut response) in std::iter::once(valid).chain(invalid).enumerate() {
        response["usage"] = json!({"prompt_tokens":2,"total_tokens":2});
        let server = ResponsesServer::start(Router::new().route(
            "/embeddings",
            post(move |Json(body): Json<Value>| {
                let response = response.clone();
                async move {
                    assert_eq!(body["input"], json!(["first", "second"]));
                    assert_eq!(body["dimensions"], 2);
                    assert_eq!(body["encoding_format"], "float");
                    Json(response)
                }
            }),
        ))
        .await;
        let client = OpenAiClient::with_embeddings_url(
            format!("http://{}/embeddings", server.address),
            crate::integration_support::prompt_catalog(),
            NonZeroUsize::new(2).unwrap(),
        );
        let result = client
            .gen_private_embeddings(&["first", "second"], Some("test"))
            .await;
        if index == 0 {
            assert_eq!(result.unwrap(), vec![vec![1.0, 0.0], vec![0.6, 0.8]]);
        } else {
            assert!(result.is_err());
        }
        server.stop().await;
    }
}

#[tokio::test]
async fn response_modes_preserve_payloads_and_streamed_deltas() {
    use axum::{routing::post, Json, Router};
    let requests = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let captured = requests.clone();
    let server = ResponsesServer::start(Router::new().route(
        "/responses",
        post(move |Json(body): Json<Value>| {
            let captured = captured.clone();
            async move {
                let streaming = body["stream"] == true;
                captured.lock().await.push(body);
                let response = if streaming {
                    concat!(
                        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"First \"}\n\n",
                        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"answer\"}\n\n",
                        "data: {\"type\":\"response.completed\",\"response\":{\"output_text\":\"Completion fallback\"}}\n\n",
                    ).to_owned()
                } else {
                    json!({"object": "response", "output_text": "First answer"}).to_string()
                };
                axum::response::Response::builder()
                    .header("content-type", if streaming { "text/event-stream" } else { "application/json" })
                    .body(axum::body::Body::from(response)).unwrap()
            }
        }),
    )).await;
    let client = server.client();
    let files = [ResponsesFileInput::FileId("file-test")];
    let buffered = client
        .gen_model_response_with_files(
            Some(" Question "),
            Some(" Instructions "),
            Some(" test-model "),
            Some(&files),
        )
        .await
        .unwrap();
    let mut deltas = Vec::new();
    let streamed = client
        .gen_model_response_with_files_streaming(
            Some(" Question "),
            Some(" Instructions "),
            Some(" test-model "),
            Some(&files),
            |delta| deltas.push(delta.to_owned()),
        )
        .await
        .unwrap();
    assert_eq!(buffered, "First answer");
    assert_eq!(streamed, buffered);
    assert_eq!(deltas, ["First ", "answer"]);
    assert_eq!(
        client.gen_model_response(None, None, None).await.unwrap(),
        buffered
    );
    assert_eq!(
        client
            .gen_model_response_with_files_streaming(None, None, None, None, |_| {})
            .await
            .unwrap(),
        buffered
    );
    let requests = requests.lock().await;
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[0]["model"], "test-model");
    assert_eq!(requests[0]["instructions"], "Instructions");
    assert_eq!(requests[0]["input"][0]["content"][1]["text"], "Question");
    let mut expected_stream = requests[0].clone();
    expected_stream["stream"] = json!(true);
    assert_eq!(requests[1], expected_stream);
    assert_eq!(requests[2]["model"], "gpt-5.5");
    assert_eq!(
        requests[2]["instructions"],
        client
            .prompts()
            .prompt(PromptId::DefaultInstructions)
            .trim()
    );
    assert_eq!(
        requests[2]["input"][0]["content"][0]["text"],
        client.prompts().prompt(PromptId::DefaultResponse).trim()
    );
    let mut expected_stream = requests[2].clone();
    expected_stream["stream"] = json!(true);
    assert_eq!(requests[3], expected_stream);
    server.stop().await;
}

#[tokio::test]
async fn response_modes_preserve_provider_status_errors() {
    use axum::{http::StatusCode, routing::post, Router};
    let server = ResponsesServer::start(Router::new().route(
        "/responses",
        post(|| async { (StatusCode::TOO_MANY_REQUESTS, "synthetic provider failure") }),
    ))
    .await;
    let client = server.client();
    let expected =
        "OpenAI responses API returned 429 Too Many Requests: synthetic provider failure";
    assert_eq!(
        client
            .gen_model_response(Some("Question"), None, None)
            .await
            .unwrap_err(),
        expected
    );
    assert_eq!(
        client
            .gen_model_response_with_files_streaming(Some("Question"), None, None, None, |_| {})
            .await
            .unwrap_err(),
        expected
    );
    server.stop().await;
}

#[tokio::test]
async fn response_modes_reject_blank_fields_before_transport() {
    let client = OpenAiClient::with_responses_url(
        "unused-test-key",
        "http://127.0.0.1:1/responses",
        crate::integration_support::prompt_catalog(),
    );
    for (prompt, instructions, model, expected) in [
        (Some(" "), Some(" "), Some(" "), "prompt cannot be empty"),
        (
            Some("Question"),
            Some(" "),
            Some(" "),
            "system instructions cannot be empty",
        ),
        (
            Some("Question"),
            Some("Instructions"),
            Some(" "),
            "model cannot be empty",
        ),
    ] {
        assert_eq!(
            client
                .gen_model_response(prompt, instructions, model)
                .await
                .unwrap_err(),
            expected
        );
        assert_eq!(
            client
                .gen_model_response_with_files_streaming(prompt, instructions, model, None, |_| {})
                .await
                .unwrap_err(),
            expected
        );
    }
}

#[test]
fn embedding_requests_use_the_configured_vector_dimension() {
    for (model, dimensions) in [
        ("text-embedding-3-small", 512),
        ("text-embedding-3-large", 3072),
    ] {
        let request = build_embeddings_request_body(
            &RigOpenAiConfig::new("unused-test-key"),
            &["document text"],
            Some(model),
            NonZeroUsize::new(dimensions).unwrap(),
        )
        .unwrap();

        assert_eq!(request["model"], model);
        assert_eq!(request["dimensions"], dimensions);
    }
}

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
        &RigOpenAiConfig::new("unused-test-key"),
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
                    "type": "message",
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
fn chat_request_replays_context_and_disables_provider_storage() {
    let context = vec![
        ChatMessageInput {
            role: ChatRole::User,
            content: "  first  ".to_string(),
        },
        ChatMessageInput {
            role: ChatRole::Assistant,
            content: "answer".to_string(),
        },
    ];
    let files = [ResponsesFileInput::ImageData {
        mime_type: "image/png",
        data_base64: "YWJj",
        detail: Some("auto"),
    }];
    let value = build_chat_request_body(
        &RigOpenAiConfig::new("unused-test-key"),
        &context,
        "next",
        "instructions",
        "model",
        &files,
    )
    .unwrap();
    assert_eq!(value["input"][0]["content"][0]["text"], "  first  ");
    assert_eq!(value["input"][0]["content"][0]["type"], "input_text");
    assert_eq!(value["input"][1]["role"], "assistant");
    assert_eq!(value["input"][1]["content"][0]["text"], "answer");
    assert_eq!(value["input"][1]["content"][0]["type"], "output_text");
    assert_eq!(value["input"][2]["content"][0]["type"], "input_image");
    assert_eq!(value["input"][2]["content"][1]["text"], "next");
    assert_eq!(value["stream"], true);
    assert_eq!(value["store"], false);
    assert!(value.get("previous_response_id").is_none());
    assert!(value.get("truncation").is_none());
}

#[test]
fn rig_requests_keep_every_inline_file_type_and_original_filename() {
    let provider = RigOpenAiConfig::new("unused-test-key");
    for (filename, mime) in [
        ("report.pdf", "application/pdf"),
        ("notes.txt", "text/plain"),
        ("notes.md", "text/markdown"),
        ("data.json", "application/json"),
        ("page.html", "text/html"),
        ("data.csv", "text/csv"),
        ("legacy.doc", "application/msword"),
        (
            "report.docx",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        ),
        (
            "deck.pptx",
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        ),
        (
            "book.xlsx",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
    ] {
        let files = [ResponsesFileInput::FileData {
            filename,
            mime_type: mime,
            data_base64: "YWJj",
        }];
        let responses = build_responses_request_body(
            &provider,
            "custom-model",
            "instructions",
            "question",
            Some(&files),
        )
        .unwrap();
        let chat = build_chat_request_body(
            &provider,
            &[],
            "question",
            "instructions",
            "custom-model",
            &files,
        )
        .unwrap();
        for body in [responses, chat] {
            assert_eq!(
                body["input"][0]["content"],
                json!([
                    {"type": "input_file", "filename": filename, "file_data": format!("data:{mime};base64,YWJj")},
                    {"type": "input_text", "text": "question"}
                ])
            );
        }
    }
    for mime in ["image/png", "image/jpeg", "image/webp", "image/gif"] {
        let files = [ResponsesFileInput::ImageData {
            mime_type: mime,
            data_base64: "YWJj",
            detail: Some("high"),
        }];
        let body = build_chat_request_body(
            &provider,
            &[],
            "question",
            "instructions",
            "custom-model",
            &files,
        )
        .unwrap();
        assert_eq!(
            body["input"][0]["content"][0],
            json!({
                "type": "input_image", "image_url": format!("data:{mime};base64,YWJj"), "detail": "high"
            })
        );
    }
}

#[test]
fn rig_encoding_keeps_request_bodies_out_of_verbose_tracing() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Writer(Arc<AtomicUsize>);
    impl std::io::Write for Writer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.fetch_add(bytes.len(), Ordering::Relaxed);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let written = Arc::new(AtomicUsize::new(0));
    let captured = written.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(move || Writer(captured.clone()))
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        tracing::trace!(target: "rig::completions", "before encoding");
        assert!(written.swap(0, Ordering::Relaxed) > 0);
        build_responses_request_body(
            &RigOpenAiConfig::new("unused-test-key"),
            "custom-model",
            "private instructions",
            "private question",
            None,
        )
        .unwrap();
        assert_eq!(written.load(Ordering::Relaxed), 0);
        tracing::trace!(target: "rig::completions", "after encoding");
        assert!(written.load(Ordering::Relaxed) > 0);
    });
}

#[test]
fn rig_requests_preserve_file_sources_and_attachment_order_after_history() {
    let provider = RigOpenAiConfig::new("unused-test-key");
    let mut file = tempfile::Builder::new().suffix(".pdf").tempfile().unwrap();
    std::io::Write::write_all(&mut file, b"abc").unwrap();
    let files = [
        ResponsesFileInput::FileId("file-test"),
        ResponsesFileInput::FileUrl("https://example.com/report.pdf"),
        ResponsesFileInput::FilePath(file.path()),
    ];
    let context = [ChatMessageInput {
        role: ChatRole::User,
        content: "previous".into(),
    }];
    let body = build_chat_request_body(
        &provider,
        &context,
        "question",
        "instructions",
        "custom-model",
        &files,
    )
    .unwrap();
    assert_eq!(body["input"][0]["content"][0]["text"], "previous");
    let content = body["input"][1]["content"].as_array().unwrap();
    assert_eq!(content.len(), 4);
    assert_eq!(
        content[0],
        json!({"type": "input_file", "file_id": "file-test"})
    );
    assert_eq!(
        content[1],
        json!({"type": "input_file", "file_url": "https://example.com/report.pdf"})
    );
    assert_eq!(
        content[2]["filename"],
        file.path().file_name().unwrap().to_str().unwrap()
    );
    assert_eq!(content[2]["file_data"], "data:application/pdf;base64,YWJj");
    assert_eq!(
        content[3],
        json!({"type": "input_text", "text": "question"})
    );
    assert_eq!(body["store"], false);
    assert!(body.get("previous_response_id").is_none());
}

#[test]
fn test_responses_url_seam_is_constructor_injected() {
    let client = OpenAiClient::with_responses_url(
        "test-key",
        "http://127.0.0.1:1/v1/responses",
        crate::integration_support::prompt_catalog(),
    );
    assert_eq!(
        client.provider.config().base_url.as_str(),
        "http://127.0.0.1:1/v1"
    );
}

#[test]
fn embedding_response_requires_the_requested_dimensions_before_storage_conversion() {
    let dimensions = NonZeroUsize::new(2).unwrap();
    let valid =
        json!({"data":[{"index":1,"embedding":[0.6,0.8]},{"index":0,"embedding":[1.0,0.0]}]});
    assert_eq!(
        extract_embeddings(&valid, 2, dimensions).unwrap(),
        vec![vec![1.0, 0.0], vec![0.6, 0.8]]
    );
    for vector in [
        json!([]),
        json!([1.0]),
        json!([1.0, 0.0, 0.0]),
        json!([f64::MAX, 0.0]),
        json!([null, 0.0]),
    ] {
        let response = json!({"data":[{"index":0,"embedding":vector}]});
        assert!(extract_embeddings(&response, 1, dimensions).is_err());
    }
    assert!(extract_embeddings(&valid, 2, NonZeroUsize::new(3).unwrap()).is_err());
}

#[tokio::test]
async fn rig_stream_accepts_large_provider_events_without_a_local_event_limit() {
    let created = format!(
        "data: {{\"type\":\"response.created\",\"padding\":\"{}\"}}\n\n",
        "x".repeat(2 * 1024 * 1024)
    );
    let done =
        "data: {\"type\":\"response.completed\",\"response\":{\"output_text\":\"answer\"}}\n\n";
    let (result, deltas) = chat_from_stream(vec![
        bytes::Bytes::from(created),
        bytes::Bytes::from_static(done.as_bytes()),
    ])
    .await;
    assert_eq!(result.unwrap(), "answer");
    assert!(deltas.is_empty());
}

impl OpenAiClient {
    pub(crate) fn with_responses_url(
        api_key: impl Into<String>,
        responses_url: impl Into<String>,
        prompts: Arc<PromptCatalog>,
    ) -> Self {
        let responses_url = responses_url.into();
        Self {
            provider: RigOpenAiConfig::new(api_key.into())
                .with_base_url(
                    responses_url
                        .strip_suffix("/responses")
                        .expect("test Responses endpoint has a suffix"),
                )
                .client(),
            embedding_dimensions: DEFAULT_EMBEDDING_DIMENSIONS,
            prompts,
        }
    }

    pub(crate) fn with_embeddings_url(
        embeddings_url: String,
        prompts: Arc<PromptCatalog>,
        dimensions: NonZeroUsize,
    ) -> Self {
        let mut client = Self::new("unused-test-key", prompts);
        client.provider = RigOpenAiConfig::new("unused-test-key")
            .with_base_url(
                embeddings_url
                    .strip_suffix("/embeddings")
                    .expect("test embedding endpoint has a suffix"),
            )
            .client();
        client.embedding_dimensions = dimensions;
        client
    }
}

fn build_embeddings_request_body(
    provider: &RigOpenAiConfig,
    contents: &[&str],
    model: Option<&str>,
    dimensions: NonZeroUsize,
) -> Result<Value, String> {
    let model = embedding_model_name(contents, model)?;

    encoded_json(
        Embeddings::new(provider.clone(), model, Some(dimensions.get()))
            .with_encoding_format(EncodingFormat::Float)
            .encode(
                contents
                    .iter()
                    .map(|content| (*content).to_owned())
                    .collect(),
                Mode::Unary,
            ),
    )
}

fn build_responses_request_body(
    provider: &RigOpenAiConfig,
    model: &str,
    system_instructions: &str,
    prompt: &str,
    file_inputs: Option<&[ResponsesFileInput<'_>]>,
) -> Result<Value, String> {
    let request = completion_request(&[], prompt, system_instructions, file_inputs, false)?;
    encode_completion(provider, model, request, Mode::Unary)
}

fn build_chat_request_body(
    provider: &RigOpenAiConfig,
    context: &[ChatMessageInput],
    prompt: &str,
    system_instructions: &str,
    model: &str,
    file_inputs: &[ResponsesFileInput<'_>],
) -> Result<Value, String> {
    encode_completion(
        provider,
        model,
        completion_request(
            context,
            prompt,
            system_instructions,
            Some(file_inputs),
            true,
        )?,
        Mode::Streaming,
    )
}

fn encode_completion(
    provider: &RigOpenAiConfig,
    model: &str,
    request: CompletionRequest,
    mode: Mode,
) -> Result<Value, String> {
    let wire = Responses::new(provider.clone(), model)
        .with_facts(ModelFacts::default().with_catalog(&Catalog::default()));
    tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
        encoded_json(wire.encode(request, mode))
    })
}

fn encoded_json(encoded: Result<Encoded, rig::error::EncodeError>) -> Result<Value, String> {
    let encoded = encoded.map_err(|_| "failed to encode OpenAI request with Rig".to_string())?;
    match encoded.request.into_body() {
        Body::Bytes(bytes) => serde_json::from_slice(&bytes)
            .map_err(|_| "Rig did not encode a JSON request".to_string()),
        Body::Multipart(_) => Err("Rig encoded an unexpected multipart request".to_string()),
    }
}
