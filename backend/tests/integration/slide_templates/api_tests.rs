use std::{
    io::{Cursor, Read},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use crate::domains::templates::slide_templates::*;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use tower::ServiceExt as _;
use zip_slide::ZipArchive;

const POWERPOINT_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.presentation";
const ONE_PIXEL_PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

fn test_app(max_json: usize, max_upload: usize) -> Router {
    compose(
        SlideTemplateEndpoint::open(SlideTemplateConfig {
            database_path: PathBuf::from(":memory:"),
            max_export_json_bytes: max_json,
            max_upload_bytes: max_upload,
            request_timeout: Duration::from_secs(5),
            preview: TemplatePreviewConfig::Disabled,
        })
        .expect("test SQLite should open"),
    )
}

fn presentation(image: bool) -> Value {
    let mut elements = vec![json!({
        "id": "title",
        "type": "text",
        "x": 80,
        "y": 80,
        "w": 500,
        "h": 80,
        "text": "Exported by the API",
        "fontSize": 30,
        "textColor": "070154"
    })];
    if image {
        elements.push(json!({
            "id": "image",
            "type": "image",
            "x": 80,
            "y": 180,
            "w": 96,
            "h": 96,
            "src": ONE_PIXEL_PNG,
            "fit": "contain"
        }));
    }
    json!({
        "presentation": {
            "title": "Export API Deck",
            "preserveElementOrder": true,
            "showBranding": false,
            "slides": [{
                "id": "slide-1",
                "name": "Exported slide",
                "width": 1280,
                "height": 720,
                "backgroundColor": "FFFFFF",
                "elements": elements
            }]
        }
    })
}

async fn send(
    app: &Router,
    request: Request<Body>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("request should complete");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), 100 * 1024 * 1024)
        .await
        .unwrap()
        .to_vec();
    (status, headers, bytes)
}

#[tokio::test]
async fn export_returns_valid_powerpoint_and_contract_headers() {
    let app = test_app(1024 * 1024, 1024 * 1024);
    let mut document = presentation(false);
    document["presentation"]["slides"][0]["elements"]
        .as_array_mut()
        .expect("elements array")
        .push(json!({
            "id": "database",
            "type": "shape",
            "shape": "flowChartMagneticDisk",
            "x": 80,
            "y": 180,
            "w": 120,
            "h": 80,
            "fill": "E8EEF8",
            "stroke": "070154",
            "strokeWidth": 1
        }));
    let request = Request::post("/api/v1/templates/slide-templates/export")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_vec(&document).expect("valid JSON"),
        ))
        .expect("valid request");
    let (status, headers, body) = send(&app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], POWERPOINT_CONTENT_TYPE);
    assert_eq!(
        headers[header::CONTENT_DISPOSITION],
        "attachment; filename=\"export-api-deck.pptx\""
    );
    assert_eq!(headers["x-powerpoint-warning-count"], "0");
    assert!(headers.contains_key("x-request-id"));
    assert_eq!(&body[..2], b"PK");

    let mut archive = ZipArchive::new(Cursor::new(body)).expect("valid OOXML ZIP");
    let mut slide_xml = String::new();
    archive
        .by_name("ppt/slides/slide1.xml")
        .expect("slide XML")
        .read_to_string(&mut slide_xml)
        .expect("UTF-8 slide XML");
    assert!(slide_xml.contains("Exported by the API"));
    assert!(slide_xml.contains(r#"prst="flowChartMagneticDisk""#));
    assert!(!slide_xml.contains(r#"prst="flowchartmagneticdisk""#));

    let mut slide_master_xml = String::new();
    archive
        .by_name("ppt/slideMasters/slideMaster1.xml")
        .expect("slide master XML")
        .read_to_string(&mut slide_master_xml)
        .expect("UTF-8 slide master XML");
    assert!(slide_master_xml.contains(r#"<p:sldLayoutId id="2147483649" r:id="rId1"/>"#));

    let mut theme_xml = String::new();
    archive
        .by_name("ppt/theme/theme1.xml")
        .expect("theme XML")
        .read_to_string(&mut theme_xml)
        .expect("UTF-8 theme XML");
    assert!(theme_xml.contains(r#"<a:fontScheme name="Arial">"#));
    assert!(theme_xml.contains(r#"<a:ea typeface=""/>"#));
    assert!(theme_xml.contains(r#"<a:cs typeface=""/>"#));
    assert_eq!(theme_xml.matches("<a:effectStyle>").count(), 3);
    assert_eq!(theme_xml.matches("<a:ln ").count(), 3);
    assert!(theme_xml.contains(r#"<a:custClr name="Highlight Magenta">"#));

    let mut view_properties_xml = String::new();
    archive
        .by_name("ppt/viewProps.xml")
        .expect("view properties XML")
        .read_to_string(&mut view_properties_xml)
        .expect("UTF-8 view properties XML");
    assert!(view_properties_xml.contains("<p:restoredLeft"));
    assert!(view_properties_xml.contains("<p:cSldViewPr"));
    assert!(view_properties_xml.contains("<p:cViewPr"));
}

#[tokio::test]
async fn connector_types_survive_export_and_import() {
    let app = test_app(1024 * 1024, 1024 * 1024);
    let mut document = presentation(false);
    let elements = document["presentation"]["slides"][0]["elements"]
        .as_array_mut()
        .expect("elements array");
    elements.extend([
        json!({
            "id": "straight-connector",
            "type": "line",
            "lineType": "straight",
            "x1": 100,
            "y1": 200,
            "x2": 300,
            "y2": 200,
            "stroke": "070154",
            "strokeWidth": 1,
            "endArrow": "triangle"
        }),
        json!({
            "id": "elbow-connector",
            "type": "line",
            "lineType": "elbow",
            "bendPosition": 0.4,
            "x1": 400,
            "y1": 200,
            "x2": 650,
            "y2": 400,
            "stroke": "070154",
            "strokeWidth": 1,
            "endArrow": "triangle"
        }),
    ]);

    let export_request = Request::post("/api/v1/templates/slide-templates/export")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_vec(&document).expect("valid JSON"),
        ))
        .expect("valid request");
    let (status, _, pptx) = send(&app, export_request).await;
    assert_eq!(status, StatusCode::OK);

    let mut archive = ZipArchive::new(Cursor::new(&pptx)).expect("valid OOXML ZIP");
    let mut slide_xml = String::new();
    archive
        .by_name("ppt/slides/slide1.xml")
        .expect("slide XML")
        .read_to_string(&mut slide_xml)
        .expect("UTF-8 slide XML");
    assert!(slide_xml.contains(r#"prst="line""#));
    assert!(slide_xml.contains(r#"prst="bentConnector2""#));
    assert!(!slide_xml.contains(r#"name="adj1" fmla="val 40000""#));
    drop(archive);

    let import_request = Request::post("/api/v1/templates/slide-templates/import")
        .header(header::CONTENT_TYPE, POWERPOINT_CONTENT_TYPE)
        .body(Body::from(pptx))
        .expect("valid request");
    let (status, _, body) = send(&app, import_request).await;
    assert_eq!(status, StatusCode::CREATED);
    let imported: Value = serde_json::from_slice(&body).expect("JSON import response");
    let imported_lines = imported["presentation"]["slides"][0]["elements"]
        .as_array()
        .expect("elements")
        .iter()
        .filter(|element| element["type"] == "line")
        .collect::<Vec<_>>();
    assert_eq!(imported_lines.len(), 2);
    assert_eq!(imported_lines[0]["lineType"], "straight");
    assert_eq!(imported_lines[1]["lineType"], "elbow");
    assert!(imported_lines[1].get("bendPosition").is_none());
    // OOXML bentConnector2 uses a horizontal-first route. Reversing endpoints
    // and arrows preserves the visible vertical-first route chosen on input.
    assert_eq!(imported_lines[1]["elbowDirection"], "horizontal-first");
    assert_eq!(imported_lines[1]["x1"].as_f64(), Some(650.0));
    assert_eq!(imported_lines[1]["y1"].as_f64(), Some(400.0));
    assert_eq!(imported_lines[1]["x2"].as_f64(), Some(400.0));
    assert_eq!(imported_lines[1]["y2"].as_f64(), Some(200.0));
    assert_eq!(imported_lines[1]["beginArrow"], "triangle");
    assert!(imported_lines[1].get("endArrow").is_none());
}

#[tokio::test]
async fn import_rejects_multi_slide_powerpoints() {
    let app = test_app(1024 * 1024, 1024 * 1024);
    let document = json!({
        "presentation": {
            "title": "Brand policy deck",
            "preserveElementOrder": true,
            "showBranding": false,
            "slides": [
                {
                    "id": "slide-1",
                    "name": "Source background",
                    "width": 1280,
                    "height": 720,
                    "backgroundColor": "FFFFFF",
                    "elements": [
                        {
                            "id": "layout-background",
                            "type": "image",
                            "x": 0,
                            "y": 0,
                            "w": 1280,
                            "h": 720,
                            "src": ONE_PIXEL_PNG,
                            "fit": "stretch",
                            "altText": "Layout Background Image"
                        },
                        {
                            "id": "west-monroe-logo",
                            "type": "image",
                            "x": 48.33,
                            "y": 664.08,
                            "w": 153,
                            "h": 32.02,
                            "src": ONE_PIXEL_PNG,
                            "fit": "stretch",
                            "altText": "West Monroe Logo"
                        },
                        {
                            "id": "plain-text",
                            "type": "text",
                            "x": 100,
                            "y": 100,
                            "w": 300,
                            "h": 60,
                            "text": "Plain text",
                            "fontFace": "Roboto",
                            "textColor": "F52C00",
                            "runs": [{
                                "text": "Plain text",
                                "fontFace": "Wingdings",
                                "fontSize": 20,
                                "color": "00B050"
                            }]
                        },
                        {
                            "id": "navy-shape",
                            "type": "shape",
                            "shape": "rect",
                            "x": 100,
                            "y": 200,
                            "w": 300,
                            "h": 60,
                            "text": "Navy shape",
                            "fill": "070154",
                            "fontFace": "Roboto",
                            "textColor": "070154",
                            "runs": [{
                                "text": "Navy shape",
                                "fontFace": "Roboto",
                                "fontSize": 20,
                                "color": "F52C00"
                            }]
                        },
                        {
                            "id": "elbow",
                            "type": "line",
                            "lineType": "elbow",
                            "bendPosition": 0.4,
                            "x1": 100,
                            "y1": 300,
                            "x2": 400,
                            "y2": 400,
                            "stroke": "070154",
                            "strokeWidth": 1
                        }
                    ]
                },
                {
                    "id": "slide-2",
                    "name": "Missing background",
                    "width": 1280,
                    "height": 720,
                    "backgroundColor": "FFFFFF",
                    "elements": [
                        {
                            "id": "light-shape",
                            "type": "shape",
                            "shape": "rect",
                            "x": 100,
                            "y": 100,
                            "w": 300,
                            "h": 60,
                            "text": "Light shape",
                            "fill": "E8EEF8",
                            "fontFace": "Aptos",
                            "textColor": "FFFFFF"
                        },
                        {
                            "id": "straight",
                            "type": "line",
                            "lineType": "straight",
                            "x1": 100,
                            "y1": 300,
                            "x2": 400,
                            "y2": 300,
                            "stroke": "070154",
                            "strokeWidth": 1
                        }
                    ]
                }
            ]
        }
    });

    let export_request = Request::post("/api/v1/templates/slide-templates/export")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_vec(&document).expect("valid JSON"),
        ))
        .expect("valid request");
    let (status, _, pptx) = send(&app, export_request).await;
    assert_eq!(status, StatusCode::OK);

    let import_request = Request::post("/api/v1/templates/slide-templates/import")
        .header(header::CONTENT_TYPE, POWERPOINT_CONTENT_TYPE)
        .body(Body::from(pptx))
        .expect("valid request");
    let (status, _, body) = send(&app, import_request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let body: Value = serde_json::from_slice(&body).expect("JSON error response");
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn import_persists_template_externalizes_asset_and_supports_reexport() {
    let database_directory = tempfile::tempdir().expect("temporary database directory");
    let database_path = database_directory.path().join("templates.sqlite");
    let app = compose(
        SlideTemplateEndpoint::open(SlideTemplateConfig {
            database_path: database_path.clone(),
            max_export_json_bytes: 1024 * 1024,
            max_upload_bytes: 1024 * 1024,
            request_timeout: Duration::from_secs(5),
            preview: TemplatePreviewConfig::Disabled,
        })
        .expect("test SQLite should open"),
    );
    let export_request = Request::post("/api/v1/templates/slide-templates/export")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_vec(&presentation(true)).expect("valid JSON"),
        ))
        .expect("valid request");
    let (_, _, pptx) = send(&app, export_request).await;

    let import_request = Request::post("/api/v1/templates/slide-templates/import")
        .header(header::CONTENT_TYPE, POWERPOINT_CONTENT_TYPE)
        .body(Body::from(pptx))
        .expect("valid request");
    let (status, headers, body) = send(&app, import_request).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "import failed: {}",
        String::from_utf8_lossy(&body)
    );
    let imported: Value = serde_json::from_slice(&body).expect("JSON import response");
    assert_eq!(imported.as_object().expect("object").len(), 1);
    let template_id = headers["x-template-id"]
        .to_str()
        .expect("template id header");
    assert_eq!(
        headers[header::LOCATION],
        format!("/api/v1/templates/slide-templates/catalog/{template_id}?appId=Quarry_WestMonroe")
    );
    assert_eq!(headers["x-powerpoint-warning-count"], "1");
    assert_eq!(headers["x-template-preview-status"], "unavailable");
    let hydrated_image = imported["presentation"]["slides"][0]["elements"]
        .as_array()
        .expect("elements")
        .iter()
        .find(|element| element["type"] == "image")
        .and_then(|element| element["src"].as_str())
        .expect("hydrated image");
    assert!(hydrated_image.starts_with("data:image/png;base64,"));

    let connection = rusqlite::Connection::open(&database_path).expect("open SQLite for assertion");
    let (asset_id, storage_type, blob_length, stored_json): (String, String, i64, String) =
        connection
            .query_row(
                "SELECT template_assets.asset_id, typeof(template_assets.asset_data),
                        length(template_assets.asset_data), templates.template_json
                 FROM template_assets
                 JOIN templates ON templates.template_id = template_assets.template_id
                 WHERE template_assets.template_id = ?1",
                [template_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("stored image BLOB");
    assert_eq!(storage_type, "blob");
    assert!(blob_length > 0);
    assert!(stored_json.contains(&format!("/import/{template_id}/assets/{asset_id}")));
    assert!(!stored_json.contains("base64"));
    let asset_path =
        format!("/api/v1/templates/slide-templates/import/{template_id}/assets/{asset_id}");

    let list_request = Request::get("/api/v1/templates/slide-templates/catalog")
        .body(Body::empty())
        .expect("valid request");
    let (status, _, listed) = send(&app, list_request).await;
    assert_eq!(status, StatusCode::OK);
    let listed: Value = serde_json::from_slice(&listed).expect("JSON template list");
    assert_eq!(listed["templates"].as_array().map(Vec::len), Some(10));
    assert_eq!(listed["templates"][0]["templateId"], template_id);
    assert_eq!(listed["templates"][0]["slideCount"], 1);
    assert_eq!(listed["templates"][0]["elementCount"], 2);
    assert!(listed["templates"][0]["title"].is_string());
    assert_eq!(listed["templates"][0]["kind"], "diagram");
    assert_eq!(listed["templates"][0]["previewUrl"], Value::Null);
    assert!(listed["templates"][0].get("document").is_none());

    connection
        .execute(
            "UPDATE templates
             SET template_json = json_set(
                 template_json,
                 '$.presentation.slides[0].elements[0].h',
                 0
             )
             WHERE template_id = ?1",
            [template_id],
        )
        .expect("seed a legacy zero-sized element");
    let get_request = Request::get(format!(
        "/api/v1/templates/slide-templates/catalog/{template_id}?appId=Quarry_WestMonroe"
    ))
    .body(Body::empty())
    .expect("valid request");
    let (status, _, retrieved) = send(&app, get_request).await;
    assert_eq!(status, StatusCode::OK);
    assert!(String::from_utf8_lossy(&retrieved).contains("base64"));
    let stored_height: f64 = connection
        .query_row(
            "SELECT json_extract(
                template_json,
                '$.presentation.slides[0].elements[0].h'
             )
             FROM templates WHERE template_id = ?1",
            [template_id],
            |row| row.get(0),
        )
        .expect("repaired stored dimension");
    assert_eq!(stored_height, 1.0);

    let asset_request = Request::get(asset_path.clone())
        .body(Body::empty())
        .expect("valid request");
    let (status, headers, asset) = send(&app, asset_request).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/png");
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert_eq!(&asset[1..4], b"PNG");

    let reexport_request = Request::post("/api/v1/templates/slide-templates/export")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_vec(&imported).expect("valid JSON"),
        ))
        .expect("valid request");
    let (status, _, reexported) = send(&app, reexport_request).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(&reexported[..2], b"PK");

    connection
        .execute(
            "DELETE FROM template_assets WHERE template_id = ?1 AND asset_id = ?2",
            rusqlite::params![template_id, asset_id],
        )
        .expect("delete test asset");
    let missing_joined_asset = Request::get(format!(
        "/api/v1/templates/slide-templates/import/{template_id}"
    ))
    .body(Body::empty())
    .expect("valid request");
    let (status, _, body) = send(&app, missing_joined_asset).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let body: Value = serde_json::from_slice(&body).expect("JSON error");
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn delete_template_removes_json_and_assets_and_returns_not_found_when_repeated() {
    let database_directory = tempfile::tempdir().expect("temporary database directory");
    let database_path = database_directory.path().join("templates.sqlite");
    let app = compose(
        SlideTemplateEndpoint::open(SlideTemplateConfig {
            database_path: database_path.clone(),
            max_export_json_bytes: 1024 * 1024,
            max_upload_bytes: 1024 * 1024,
            request_timeout: Duration::from_secs(5),
            preview: TemplatePreviewConfig::Disabled,
        })
        .expect("test SQLite should open"),
    );
    let template_id = "template-to-delete";
    let asset_id = "asset-to-delete";
    let connection = rusqlite::Connection::open(&database_path).expect("open SQLite for seeding");
    connection
        .execute(
            "INSERT INTO templates (template_id, template_json) VALUES (?1, json(?2))",
            rusqlite::params![
                template_id,
                serde_json::to_string(&presentation(false)).expect("serialize presentation")
            ],
        )
        .expect("seed template");
    connection
        .execute(
            "INSERT INTO template_assets (asset_id, template_id, content_type, asset_data)
             VALUES (?1, ?2, 'image/png', ?3)",
            rusqlite::params![asset_id, template_id, [1_u8, 2, 3]],
        )
        .expect("seed template asset");
    connection
        .execute(
            "INSERT INTO app_templates VALUES ('Quarry_WestMonroe',?1,CURRENT_TIMESTAMP)",
            [template_id],
        )
        .expect("grant seeded template to default app");

    let delete_request = Request::delete(format!(
        "/api/v1/templates/slide-templates/catalog/{template_id}?appId=Quarry_WestMonroe"
    ))
    .body(Body::empty())
    .expect("valid request");
    let (status, headers, body) = send(&app, delete_request).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(headers.contains_key("x-request-id"));
    assert!(body.is_empty());

    let template_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM templates WHERE template_id = ?1",
            [template_id],
            |row| row.get(0),
        )
        .expect("count deleted template");
    let asset_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM template_assets WHERE template_id = ?1",
            [template_id],
            |row| row.get(0),
        )
        .expect("count cascaded assets");
    assert_eq!(template_count, 0);
    assert_eq!(asset_count, 0);

    let repeated_request = Request::delete(format!(
        "/api/v1/templates/slide-templates/catalog/{template_id}?appId=Quarry_WestMonroe"
    ))
    .body(Body::empty())
    .expect("valid request");
    let (status, _, body) = send(&app, repeated_request).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let body: Value = serde_json::from_slice(&body).expect("JSON error");
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn built_in_catalog_filters_by_kind_and_serves_previews() {
    let app = test_app(1024 * 1024, 1024 * 1024);
    let request = Request::get("/api/v1/templates/slide-templates/catalog?kind=commentary")
        .body(Body::empty())
        .expect("valid request");
    let (status, _, body) = send(&app, request).await;
    assert_eq!(status, StatusCode::OK);
    let catalog: Value = serde_json::from_slice(&body).expect("catalog JSON");
    let templates = catalog["templates"].as_array().expect("templates");
    assert_eq!(templates.len(), 3);
    assert!(templates
        .iter()
        .all(|template| template["kind"] == "commentary"));
    let preview_url = templates[0]["previewUrl"].as_str().expect("preview URL");

    let preview_request = Request::get(preview_url.to_owned())
        .body(Body::empty())
        .expect("valid request");
    let (status, headers, preview) = send(&app, preview_request).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/png");
    assert_eq!(headers[header::CACHE_CONTROL], "private, no-store");
    assert!(preview.starts_with(b"\x89PNG\r\n\x1a\n"));

    let invalid_request = Request::get("/api/v1/templates/slide-templates/catalog?kind=unknown")
        .body(Body::empty())
        .expect("valid request");
    let (status, _, body) = send(&app, invalid_request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let error: Value = serde_json::from_slice(&body).expect("error JSON");
    assert!(error["error"].is_string());
}

struct FakePreview;

impl TemplatePreviewGenerator for FakePreview {
    fn generate(&self, _source: &[u8]) -> anyhow::Result<Option<GeneratedTemplatePreview>> {
        let bytes = STANDARD.decode(ONE_PIXEL_PNG.split_once(',').expect("data URI").1)?;
        Ok(Some(GeneratedTemplatePreview {
            bytes,
            width: 1,
            height: 1,
        }))
    }
}

#[tokio::test]
async fn import_persists_preview_and_selected_kind() {
    let directory = tempfile::tempdir().unwrap();
    let config = SlideTemplateConfig {
        database_path: directory.path().join("templates.sqlite"),
        max_export_json_bytes: 1024 * 1024,
        max_upload_bytes: 1024 * 1024,
        request_timeout: Duration::from_secs(5),
        preview: TemplatePreviewConfig::Disabled,
    };
    let repository = Arc::new(
        crate::slide_template_test_support::repository(&config.database_path)
            .expect("persistent repository"),
    );
    let endpoint = SlideTemplateEndpoint::with_repository_and_preview(
        config.clone(),
        repository,
        Arc::new(FakePreview),
    );
    let app = compose(endpoint);
    let export_request = Request::post("/api/v1/templates/slide-templates/export")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_vec(&presentation(true)).expect("JSON"),
        ))
        .expect("valid request");
    let (_, _, pptx) = send(&app, export_request).await;
    let import_request = Request::post("/api/v1/templates/slide-templates/import?kind=commentary")
        .header(header::CONTENT_TYPE, POWERPOINT_CONTENT_TYPE)
        .body(Body::from(pptx))
        .expect("valid request");
    let (status, headers, imported_body) = send(&app, import_request).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(headers["x-template-preview-status"], "ready");
    assert!(headers.contains_key(header::LINK));
    let template_id = headers["x-template-id"]
        .to_str()
        .expect("template ID")
        .to_owned();

    let list_request = Request::get("/api/v1/templates/slide-templates/catalog?kind=commentary")
        .body(Body::empty())
        .expect("valid request");
    let (_, _, list_body) = send(&app, list_request).await;
    let list: Value = serde_json::from_slice(&list_body).expect("list JSON");
    assert_eq!(list["templates"].as_array().map(Vec::len), Some(1));
    assert_eq!(list["templates"][0]["templateId"], template_id);
    assert!(list["templates"][0]["previewUrl"].is_string());

    drop(app);
    let app = compose(SlideTemplateEndpoint::open(config).unwrap());
    let (status, _, body) = send(
        &app,
        Request::get(format!(
            "/api/v1/templates/slide-templates/catalog/{template_id}"
        ))
        .body(Body::empty())
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap(),
        serde_json::from_slice::<Value>(&imported_body).unwrap()
    );
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert!(body["presentation"]["slides"][0]["elements"]
        .as_array()
        .unwrap()
        .iter()
        .any(|element| element["src"]
            .as_str()
            .is_some_and(|src| src.starts_with("data:image/png;base64,"))));

    let preview_request = Request::get(format!(
        "/api/v1/templates/slide-templates/catalog/{template_id}/preview"
    ))
    .body(Body::empty())
    .expect("valid request");
    let (status, _, bytes) = send(&app, preview_request).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes.len(), 68);
}

#[tokio::test]
async fn batch_import_stores_each_slide_and_lists_paginated_preview_data() {
    let repository = Arc::new(
        crate::slide_template_test_support::repository(PathBuf::from(":memory:").as_path())
            .expect("in-memory repository"),
    );
    let endpoint = SlideTemplateEndpoint::with_repository_and_preview(
        SlideTemplateConfig {
            database_path: PathBuf::from(":memory:"),
            max_export_json_bytes: 2 * 1024 * 1024,
            max_upload_bytes: 2 * 1024 * 1024,
            request_timeout: Duration::from_secs(5),
            preview: TemplatePreviewConfig::Disabled,
        },
        repository,
        Arc::new(FakePreview),
    );
    let app = compose(endpoint);
    let mut document = presentation(false);
    let base_slide = document["presentation"]["slides"][0].clone();
    document["presentation"]["slides"] = Value::Array(
        (1..=12)
            .map(|index| {
                let mut slide = base_slide.clone();
                slide["id"] = Value::String(format!("slide-{index}"));
                slide["name"] = Value::String(format!("Batch slide {index}"));
                slide["elements"][0]["id"] = Value::String(format!("title-{index}"));
                slide["elements"][0]["text"] = Value::String(format!("Batch slide {index}"));
                slide
            })
            .collect(),
    );
    let export_request = Request::post("/api/v1/templates/slide-templates/export")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_vec(&document).expect("presentation JSON"),
        ))
        .expect("valid request");
    let (status, _, pptx) = send(&app, export_request).await;
    assert_eq!(status, StatusCode::OK);

    let import_request =
        Request::post("/api/v1/templates/slide-templates/batch-import?kind=commentary")
            .header(header::CONTENT_TYPE, POWERPOINT_CONTENT_TYPE)
            .body(Body::from(pptx))
            .expect("valid request");
    let (status, headers, body) = send(&app, import_request).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(headers["x-imported-template-count"], "12");
    assert_eq!(headers["x-powerpoint-warning-count"], "0");
    let imported: Value = serde_json::from_slice(&body).expect("batch import JSON");
    assert_eq!(imported["templates"].as_array().map(Vec::len), Some(12));
    assert!(imported["templates"].as_array().is_some_and(|templates| {
        templates.iter().all(|template| {
            template["previewAvailable"] == true
                && template["templateJson"]["presentation"]["slides"]
                    .as_array()
                    .is_some_and(|slides| slides.len() == 1)
        })
    }));

    let first_page_request = Request::get("/api/v1/templates/slide-templates/catalog/previews")
        .body(Body::empty())
        .expect("valid request");
    let (status, headers, body) = send(&app, first_page_request).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CACHE_CONTROL], "private, no-store");
    let first_page: Value = serde_json::from_slice(&body).expect("preview page JSON");
    assert_eq!(first_page["previews"].as_array().map(Vec::len), Some(10));
    assert_eq!(first_page["pagination"]["page"], 1);
    assert_eq!(first_page["pagination"]["pageSize"], 10);
    assert_eq!(first_page["pagination"]["totalItems"], 12);
    assert_eq!(first_page["pagination"]["totalPages"], 2);
    assert_eq!(first_page["pagination"]["hasPreviousPage"], false);
    assert_eq!(first_page["pagination"]["hasNextPage"], true);
    assert!(first_page["previews"].as_array().is_some_and(|previews| {
        previews.iter().all(|preview| {
            preview["contentType"] == "image/png"
                && preview["dataUrl"]
                    .as_str()
                    .is_some_and(|url| url.starts_with("data:image/png;base64,"))
        })
    }));

    let second_page_request =
        Request::get("/api/v1/templates/slide-templates/catalog/previews?page=2")
            .body(Body::empty())
            .expect("valid request");
    let (_, _, body) = send(&app, second_page_request).await;
    let second_page: Value = serde_json::from_slice(&body).expect("preview page JSON");
    assert_eq!(second_page["previews"].as_array().map(Vec::len), Some(2));
    assert_eq!(second_page["pagination"]["hasPreviousPage"], true);
    assert_eq!(second_page["pagination"]["hasNextPage"], false);

    for page in ["0", "-1", "1.5", "not-a-number", "18446744073709551616"] {
        let request = Request::get(format!(
            "/api/v1/templates/slide-templates/catalog/previews?page={page}"
        ))
        .body(Body::empty())
        .expect("valid request");
        let (status, _, body) = send(&app, request).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let error: Value = serde_json::from_slice(&body).expect("error JSON");
        assert!(error["error"].is_string());
    }
    let repeated_page =
        Request::get("/api/v1/templates/slide-templates/catalog/previews?page=1&page=2")
            .body(Body::empty())
            .expect("valid request");
    let (status, _, body) = send(&app, repeated_page).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let error: Value = serde_json::from_slice(&body).expect("error JSON");
    assert!(error["error"].is_string());
}

#[tokio::test]
async fn insert_endpoint_adds_generated_slide_and_clamps_position_to_target_deck() {
    let app = test_app(1024 * 1024, 1024 * 1024);
    let json = serde_json::to_vec(&presentation(false)).expect("presentation JSON");
    let export_request = Request::post("/api/v1/templates/slide-templates/export")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json.clone()))
        .expect("valid request");
    let (_, _, target) = send(&app, export_request).await;
    let boundary = "slide-template-test-boundary";
    let body = multipart_body(boundary, &json, &target, "99");
    let request = Request::post("/api/v1/templates/slide-templates/export/insert")
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .expect("valid request");
    let (status, headers, inserted) = send(&app, request).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "insert failed: {}",
        String::from_utf8_lossy(&inserted)
    );
    assert_eq!(headers[header::CONTENT_TYPE], POWERPOINT_CONTENT_TYPE);
    assert!(headers[header::CONTENT_DISPOSITION]
        .to_str()
        .expect("content disposition")
        .contains("target-with-slide.pptx"));

    let mut archive = ZipArchive::new(Cursor::new(inserted)).expect("inserted PPTX ZIP");
    let mut presentation_xml = String::new();
    archive
        .by_name("ppt/presentation.xml")
        .expect("presentation XML")
        .read_to_string(&mut presentation_xml)
        .expect("UTF-8 presentation XML");
    assert_eq!(presentation_xml.matches("<p:sldId ").count(), 2);
    assert!(archive.by_name("ppt/slides/slide2.xml").is_ok());
}

fn multipart_body(boundary: &str, presentation: &[u8], target: &[u8], position: &str) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(
        format!("--{boundary}\r\nContent-Disposition: form-data; name=\"presentation\"\r\n\r\n")
            .as_bytes(),
    );
    body.extend_from_slice(presentation);
    body.extend_from_slice(format!("\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"target\"; filename=\"target.pptx\"\r\nContent-Type: {POWERPOINT_CONTENT_TYPE}\r\n\r\n").as_bytes());
    body.extend_from_slice(target);
    body.extend_from_slice(format!("\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"insertAfterSlide\"\r\n\r\n{position}\r\n--{boundary}--\r\n").as_bytes());
    body
}

#[derive(Clone)]
struct SlideTemplateConfig {
    database_path: PathBuf,
    max_export_json_bytes: usize,
    max_upload_bytes: usize,
    request_timeout: Duration,
    preview: TemplatePreviewConfig,
}
#[derive(Clone)]
enum TemplatePreviewConfig {
    Disabled,
}
struct SlideTemplateEndpoint {
    studio: Arc<SlideTemplates>,
}
impl SlideTemplateEndpoint {
    fn open(config: SlideTemplateConfig) -> anyhow::Result<Self> {
        let repository = Arc::new(crate::slide_template_test_support::repository(
            &config.database_path,
        )?);
        crate::domains::templates::slide_templates::catalog::seed(repository.as_ref())?;
        Ok(Self::with_repository_and_preview(
            config,
            repository,
            Arc::new(DisabledTemplatePreviewGenerator),
        ))
    }
    fn with_repository_and_preview(
        config: SlideTemplateConfig,
        repository: Arc<dyn TemplateRepository>,
        preview: Arc<dyn TemplatePreviewGenerator>,
    ) -> Self {
        let _ = (
            config.max_export_json_bytes,
            config.max_upload_bytes,
            config.request_timeout,
            config.preview,
        );
        Self {
            studio: Arc::new(SlideTemplates::new(repository, preview, None, None, false)),
        }
    }
}
fn compose(endpoint: SlideTemplateEndpoint) -> Router {
    let studio = endpoint.studio;
    let facade =
        Arc::new(crate::domains::templates::service::TemplateService::embedded(studio.clone()));
    let api = crate::domains::templates::route::routes(facade).merge(
        crate::domains::templates::slide_templates::route::routes(studio),
    );
    crate::create_router(api, &crate::AppConfig::default().http)
}
