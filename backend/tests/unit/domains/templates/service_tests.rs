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
        service.delete("example").await.unwrap_err(),
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

#[tokio::test]
async fn pptx_import_rejects_empty_bytes_before_capability_lookup() {
    let service = TemplateService::new(None);

    assert_eq!(
        service
            .import_pptx_template(
                PptxTemplateUpload { bytes: Vec::new() },
                PptxTemplateImportMode::Single,
            )
            .await
            .unwrap_err(),
        ServiceError::Validation(
            "PPTX template bytes are empty or exceed the 25 MB limit".to_string()
        )
    );
}

#[test]
fn single_slide_rejection_maps_to_the_actionable_product_error() {
    assert_eq!(
        map_pptx_import_client_error(
            TemplateClientError::PptxTemplateMustHaveOneSlide,
            PptxTemplateImportMode::Single,
        ),
        ServiceError::Validation(PPTX_MULTI_SLIDE_IMPORT_MESSAGE.to_string())
    );
}

#[test]
fn transport_failure_maps_to_an_uncertain_outcome_without_upstream_details() {
    assert_eq!(
        map_pptx_import_client_error(
            TemplateClientError::Request("private URL".to_string()),
            PptxTemplateImportMode::Batch,
        ),
        ServiceError::Unavailable(
            "Template import could not be confirmed. Refresh templates before trying again."
                .to_string()
        )
    );
}

#[tokio::test]
async fn delete_rejects_an_invalid_template_id_before_capability_lookup() {
    let service = TemplateService::new(None);

    assert_eq!(
        service.delete("   ").await.unwrap_err(),
        ServiceError::Validation("template ID is invalid".to_string())
    );
}

#[test]
fn upstream_missing_template_maps_to_not_found() {
    assert_eq!(
        map_delete_client_error(TemplateClientError::Status(reqwest::StatusCode::NOT_FOUND,)),
        ServiceError::NotFound("template was not found".to_string())
    );
}

#[tokio::test]
async fn service_rejects_pages_beyond_the_catalog_guard() {
    let service = TemplateService::new(None);

    assert_eq!(
        service
            .list_previews(MAX_TEMPLATE_PREVIEW_PAGES + 1)
            .await
            .unwrap_err(),
        ServiceError::Validation("page is outside the supported range".to_string())
    );
}

#[test]
fn adapter_response_maps_to_the_camel_case_product_contract() {
    let page = TemplatePreviewPage::from(AdapterPage {
        pagination: crate::adapters::diligence_studio::templates::TemplatePreviewPagination {
            page: 1,
            page_size: 10,
            total_items: 1,
            total_pages: 1,
            has_next_page: false,
            has_previous_page: false,
        },
        previews: vec![
            crate::adapters::diligence_studio::templates::TemplatePreview {
                template_id: "example".to_string(),
                content_type: "image/png".to_string(),
                data_url: "data:image/png;base64,example".to_string(),
                preview_url: "/templates/example/preview?appId=Quarry_WestMonroe".to_string(),
                width: 1600,
                height: 900,
            },
        ],
    });

    let json = serde_json::to_value(page).unwrap();
    assert_eq!(json["pagination"]["pageSize"], 10);
    assert_eq!(json["previews"][0]["templateId"], "example");
    assert_eq!(
        json["previews"][0]["dataUrl"],
        "data:image/png;base64,example"
    );
}
