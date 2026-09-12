use super::*;

use axum::{
    body::{Body, Bytes},
    extract::Query,
    http::{HeaderMap, Response, StatusCode},
    routing::{delete, get, post},
    Json, Router,
};
use image::{DynamicImage, ImageOutputFormat, RgbaImage};
use serde_json::{json, Value};
use std::collections::HashMap;

struct TestServer {
    address: std::net::SocketAddr,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[tokio::test]
async fn requests_the_typed_versioned_endpoint_and_preserves_payload() {
    async fn previews(
        headers: HeaderMap,
        Query(query): Query<HashMap<String, String>>,
    ) -> Json<Value> {
        assert_eq!(
            headers
                .get(APP_ID_HEADER)
                .and_then(|value| value.to_str().ok()),
            Some(DILIGENCE_STUDIO_APP_ID)
        );
        let page = query
            .get("page")
            .and_then(|value| value.parse().ok())
            .unwrap();
        Json(valid_payload(page, 2, 2))
    }

    let server = spawn(Router::new().route("/api/v1/templates/previews", get(previews))).await;
    let client = DiligenceStudioClient::new(
        reqwest::Client::new(),
        reqwest::Url::parse(&format!("http://{}/api/v1/", server.address)).unwrap(),
    );

    let response = client.list_template_previews(2).await.unwrap();

    assert_eq!(response.pagination.page, 2);
    assert_eq!(response.previews[0].template_id, "template-2");
    assert!(response.previews[0]
        .data_url
        .starts_with(PNG_DATA_URL_PREFIX));
}

#[tokio::test]
async fn deletes_an_encoded_template_for_the_quarry_app() {
    async fn remove(headers: HeaderMap) -> StatusCode {
        assert_eq!(
            headers
                .get(APP_ID_HEADER)
                .and_then(|value| value.to_str().ok()),
            Some(DILIGENCE_STUDIO_APP_ID)
        );
        StatusCode::NO_CONTENT
    }

    let server =
        spawn(Router::new().route("/api/v1/templates/template%2Fone", delete(remove))).await;
    let client = DiligenceStudioClient::new(
        reqwest::Client::new(),
        reqwest::Url::parse(&format!("http://{}/api/v1/", server.address)).unwrap(),
    );

    client.delete_template("template/one").await.unwrap();
}

#[tokio::test]
async fn delete_requires_the_upstream_204_contract() {
    async fn invalid_response() -> StatusCode {
        StatusCode::OK
    }

    let server =
        spawn(Router::new().route("/api/v1/templates/example", delete(invalid_response))).await;
    let client = DiligenceStudioClient::new(
        reqwest::Client::new(),
        reqwest::Url::parse(&format!("http://{}/api/v1/", server.address)).unwrap(),
    );

    assert!(matches!(
        client.delete_template("example").await.unwrap_err(),
        TemplateClientError::InvalidPayload(_)
    ));
}

#[tokio::test]
async fn imports_one_slide_with_raw_powerpoint_bytes_and_required_headers() {
    async fn import(
        headers: HeaderMap,
        Query(query): Query<HashMap<String, String>>,
        body: Bytes,
    ) -> Response<Body> {
        assert_eq!(query.get("kind").map(String::as_str), Some("diagram"));
        assert_eq!(headers.get(APP_ID_HEADER).unwrap(), DILIGENCE_STUDIO_APP_ID);
        assert_eq!(headers.get(CONTENT_TYPE).unwrap(), POWERPOINT_CONTENT_TYPE);
        assert_eq!(body.as_ref(), b"powerpoint bytes");
        Response::builder()
            .status(StatusCode::CREATED)
            .header(CONTENT_TYPE, "application/json; charset=utf-8")
            .header("x-template-id", "template-123")
            .header("x-template-preview-status", "ready")
            .header("x-powerpoint-warning-count", "2")
            .body(Body::from("{}"))
            .unwrap()
    }

    let server = spawn(Router::new().route("/api/v1/import", post(import))).await;
    let client = client_for(&server);

    assert_eq!(
        client
            .import_pptx_template(b"powerpoint bytes".to_vec(), PptxTemplateImportMode::Single)
            .await
            .unwrap(),
        PptxTemplateImportResult {
            imported_count: 1,
            warning_count: 2,
        }
    );
}

#[tokio::test]
async fn imports_a_deck_directly_through_the_batch_endpoint() {
    async fn batch(
        headers: HeaderMap,
        Query(query): Query<HashMap<String, String>>,
        body: Bytes,
    ) -> Response<Body> {
        assert_eq!(query.get("kind").map(String::as_str), Some("diagram"));
        assert_eq!(headers.get(APP_ID_HEADER).unwrap(), DILIGENCE_STUDIO_APP_ID);
        assert_eq!(headers.get(CONTENT_TYPE).unwrap(), POWERPOINT_CONTENT_TYPE);
        assert_eq!(body.as_ref(), b"deck bytes");
        Response::builder()
            .status(StatusCode::CREATED)
            .header(CONTENT_TYPE, "application/json")
            .header("x-imported-template-count", "4")
            .header("x-powerpoint-warning-count", "0")
            .body(Body::from("{}"))
            .unwrap()
    }

    let server = spawn(Router::new().route("/api/v1/batchImport", post(batch))).await;
    let client = client_for(&server);

    assert_eq!(
        client
            .import_pptx_template(b"deck bytes".to_vec(), PptxTemplateImportMode::Batch)
            .await
            .unwrap(),
        PptxTemplateImportResult {
            imported_count: 4,
            warning_count: 0,
        }
    );
}

#[tokio::test]
async fn preserves_the_exact_single_slide_rejection_without_batch_fallback() {
    async fn reject() -> (StatusCode, Json<Value>) {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "error": {
                    "code": "template_must_have_one_slide",
                    "message": "private upstream message"
                }
            })),
        )
    }

    let server = spawn(Router::new().route("/api/v1/import", post(reject))).await;
    let client = client_for(&server);

    assert!(matches!(
        client
            .import_pptx_template(vec![1], PptxTemplateImportMode::Single)
            .await
            .unwrap_err(),
        TemplateClientError::PptxTemplateMustHaveOneSlide
    ));
}

#[test]
fn import_success_headers_are_bounded_and_mode_specific() {
    let single_with_batch_header = HeaderMap::from_iter([
        (
            "content-type".parse().unwrap(),
            "application/json".parse().unwrap(),
        ),
        (
            "x-template-id".parse().unwrap(),
            "template-1".parse().unwrap(),
        ),
        (
            "x-template-preview-status".parse().unwrap(),
            "ready".parse().unwrap(),
        ),
        (
            "x-powerpoint-warning-count".parse().unwrap(),
            "0".parse().unwrap(),
        ),
        (
            "x-imported-template-count".parse().unwrap(),
            "1".parse().unwrap(),
        ),
    ]);
    assert!(validate_pptx_import_headers(
        &single_with_batch_header,
        PptxTemplateImportMode::Single
    )
    .is_err());

    let excessive_batch = HeaderMap::from_iter([
        (
            "x-imported-template-count".parse().unwrap(),
            "1001".parse().unwrap(),
        ),
        (
            "x-powerpoint-warning-count".parse().unwrap(),
            "0".parse().unwrap(),
        ),
    ]);
    assert!(validate_pptx_import_headers(&excessive_batch, PptxTemplateImportMode::Batch).is_err());
}

#[test]
fn rejects_invalid_preview_and_pagination_contracts() {
    let mut invalid_content_type = upstream_page(1, 1, 1);
    invalid_content_type.previews[0].content_type = "image/jpeg".to_string();
    assert!(validate_page(invalid_content_type, 1).is_err());

    let mut unsafe_url = upstream_page(1, 1, 1);
    unsafe_url.previews[0].preview_url = "https://evil.example/preview".to_string();
    assert!(validate_page(unsafe_url, 1).is_err());

    let mut wrong_app = upstream_page(1, 1, 1);
    wrong_app.previews[0].preview_url =
        "/templates/example/preview?appId=DiligenceStudio_WestMonroe".to_string();
    assert!(validate_page(wrong_app, 1).is_err());

    let mut mismatched_dimensions = upstream_page(1, 1, 1);
    mismatched_dimensions.previews[0].width = 2;
    assert!(validate_page(mismatched_dimensions, 1).is_err());

    let mut invalid_pagination = upstream_page(1, 1, 1);
    invalid_pagination.pagination.has_next_page = true;
    assert!(validate_page(invalid_pagination, 1).is_err());

    let mut duplicate = upstream_page(1, 2, 1);
    duplicate.pagination.page_size = 10;
    duplicate.previews.push(
        serde_json::from_value(json!({
            "templateId": "template-1",
            "contentType": "image/png",
            "dataUrl": png_data_url(),
            "previewUrl": "/templates/duplicate/preview?appId=Quarry_WestMonroe",
            "width": 1,
            "height": 1
        }))
        .unwrap(),
    );
    assert!(validate_page(duplicate, 1).is_err());
}

#[test]
fn accepts_the_explicit_empty_catalog_shape() {
    let page = UpstreamPage {
        pagination: UpstreamPagination {
            page: 1,
            page_size: 10,
            total_items: 0,
            total_pages: 0,
            has_next_page: false,
            has_previous_page: false,
        },
        previews: Vec::new(),
    };

    assert!(validate_page(page, 1).unwrap().previews.is_empty());
}

async fn spawn(router: Router) -> TestServer {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    TestServer { address, task }
}

fn client_for(server: &TestServer) -> DiligenceStudioClient {
    DiligenceStudioClient::new(
        reqwest::Client::new(),
        reqwest::Url::parse(&format!("http://{}/api/v1/", server.address)).unwrap(),
    )
}

fn valid_payload(page: usize, total_items: usize, total_pages: usize) -> Value {
    json!({
        "pagination": {
            "page": page,
            "pageSize": 1,
            "totalItems": total_items,
            "totalPages": total_pages,
            "hasNextPage": page < total_pages,
            "hasPreviousPage": page > 1
        },
        "previews": [{
            "templateId": format!("template-{page}"),
            "contentType": "image/png",
            "dataUrl": png_data_url(),
            "previewUrl": format!(
                "/templates/template-{page}/preview?appId={DILIGENCE_STUDIO_APP_ID}"
            ),
            "width": 1,
            "height": 1
        }]
    })
}

fn upstream_page(page: usize, total_items: usize, total_pages: usize) -> UpstreamPage {
    serde_json::from_value(valid_payload(page, total_items, total_pages)).unwrap()
}

fn png_data_url() -> String {
    let mut bytes = Vec::new();
    DynamicImage::ImageRgba8(RgbaImage::new(1, 1))
        .write_to(&mut Cursor::new(&mut bytes), ImageOutputFormat::Png)
        .unwrap();
    format!(
        "{PNG_DATA_URL_PREFIX}{}",
        general_purpose::STANDARD.encode(bytes)
    )
}
