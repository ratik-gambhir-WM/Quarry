use super::*;

#[tokio::test]
async fn unconfigured_service_returns_a_sanitized_unavailable_error() {
    let service = TemplateService::new(None);

    assert_eq!(
        service.list_previews(1).await.unwrap_err(),
        ServiceError::Unavailable("template preview capability is not configured".to_string())
    );
}

#[tokio::test]
async fn unconfigured_delete_returns_a_sanitized_unavailable_error() {
    let service = TemplateService::new(None);

    assert_eq!(
        service.delete("example", 1).await.unwrap_err(),
        ServiceError::Unavailable("template capability is not configured".to_string())
    );
}

#[tokio::test]
async fn unconfigured_get_returns_a_sanitized_unavailable_error() {
    let service = TemplateService::new(None);

    assert_eq!(
        service.get("example").await.unwrap_err(),
        ServiceError::Unavailable("template capability is not configured".to_string())
    );
}

#[tokio::test]
async fn unconfigured_import_returns_a_sanitized_unavailable_error() {
    let service = TemplateService::new(None);

    assert_eq!(
        service
            .import_pptx_template(
                PptxTemplateUpload { bytes: vec![1] },
                PptxTemplateImportMode::Single,
            )
            .await
            .unwrap_err(),
        ServiceError::Unavailable("template import is not configured".to_string())
    );
}

#[test]
fn service_rejects_a_powerpoint_result_above_the_response_limit() {
    assert_eq!(
        validate_powerpoint_export_size(MAX_POWERPOINT_EXPORT_BYTES),
        Ok(())
    );
    assert_eq!(
        validate_powerpoint_export_size(MAX_POWERPOINT_EXPORT_BYTES + 1),
        Err(ServiceError::Unavailable(
            "PowerPoint export exceeded the 64 MB limit".to_string()
        ))
    );
}

#[test]
fn single_slide_rejection_maps_to_the_actionable_product_error() {
    assert_eq!(
        map_embedded_error(super::super::slide_templates::SlideTemplateError::new(
            super::super::slide_templates::error::FailureKind::Invalid,
            "template_must_have_one_slide",
            "Template PowerPoint files must contain exactly one slide.",
        )),
        ServiceError::Validation(PPTX_MULTI_SLIDE_IMPORT_MESSAGE.to_string())
    );
}

#[test]
fn embedded_internal_errors_do_not_expose_their_source() {
    assert_eq!(
        map_embedded_error(super::super::slide_templates::SlideTemplateError::internal(
            anyhow::anyhow!("private storage detail"),
        )),
        ServiceError::Unavailable("The request could not be completed.".to_string())
    );
}

#[test]
fn embedded_missing_templates_preserve_not_found() {
    assert_eq!(
        map_embedded_error(super::super::slide_templates::SlideTemplateError::new(
            super::super::slide_templates::error::FailureKind::NotFound,
            "template_not_found",
            "template was not found",
        )),
        ServiceError::NotFound("template was not found".to_string())
    );
}

#[test]
fn preview_response_preserves_the_camel_case_product_contract() {
    let page = TemplatePreviewPage {
        pagination: TemplatePreviewPagination {
            page: 1,
            page_size: 10,
            total_items: 1,
            total_pages: 1,
            has_next_page: false,
            has_previous_page: false,
        },
        previews: vec![TemplatePreview {
            revision: 1,
            template_id: "example".to_string(),
            content_type: "image/png".to_string(),
            data_url: "data:image/png;base64,example".to_string(),
            preview_url: "/api/v1/templates/slide-templates/import/example/preview".to_string(),
            width: 1600,
            height: 900,
        }],
    };

    let json = serde_json::to_value(page).unwrap();
    assert_eq!(json["pagination"]["pageSize"], 10);
    assert_eq!(json["previews"][0]["templateId"], "example");
    assert_eq!(
        json["previews"][0]["dataUrl"],
        "data:image/png;base64,example"
    );
}
