use super::*;
use crate::domains::templates::slide_templates::{
    StoredTemplate, StoredTemplateMetadata, TemplateKind, TemplateSource,
};
use serde_json::json;

fn document(sources: &[&str]) -> CanvasDocument {
    serde_json::from_value(json!({
        "presentation": {
            "title": "Asset fixture", "preserveElementOrder": true, "showBranding": false,
            "slides": [{
                "id": "slide", "name": "Asset fixture", "width": 1280, "height": 720,
                "backgroundColor": "FFFFFF",
                "elements": sources.iter().enumerate().map(|(index, source)| json!({
                    "type": "image", "id": format!("image-{index}"),
                    "x": 0, "y": 0, "w": 10, "h": 10, "src": source, "fit": "contain"
                })).collect::<Vec<_>>()
            }]
        }
    }))
    .unwrap()
}

async fn store_assets(
    document: CanvasDocument,
    assets: &[TemplateAsset],
) -> impl TemplateRepository {
    let repository =
        crate::slide_template_test_support::repository(std::path::Path::new(":memory:"))
            .await
            .unwrap();
    repository
        .insert(
            &StoredTemplate {
                revision: 1,
                template_id: "template".into(),
                template_json: document,
            },
            assets,
            &StoredTemplateMetadata {
                template_id: "template".into(),
                kind: TemplateKind::Diagram,
                title: "Asset fixture".into(),
                description: String::new(),
                source: TemplateSource::Import,
                created_at: 0,
                catalog_version: None,
                catalog_checksum: None,
            },
            None,
        )
        .await
        .unwrap();
    repository
}

#[tokio::test]
async fn asset_externalization_deduplicates_sources_and_round_trips_both_hydration_paths() {
    let original = document(&["data:image/png;base64,AQID", "data:image/png;base64,AQID"]);
    let mut next_id = 0;
    let (stored, assets) = externalize_assets_with("template", original.clone(), || {
        next_id += 1;
        format!("asset-{next_id}")
    })
    .unwrap();
    assert_eq!(next_id, 1);
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].bytes, [1, 2, 3]);
    assert_eq!(
        hydrate_canvas_assets(stored.clone(), &assets).unwrap(),
        original
    );
    let repository = store_assets(stored.clone(), &assets).await;
    let mut exported = stored;
    hydrate_template_assets(&mut exported, &repository)
        .await
        .unwrap();
    assert_eq!(exported, original);
}

#[tokio::test]
async fn asset_hydration_requires_the_matching_template_and_asset_ids() {
    let assets = vec![TemplateAsset {
        template_id: "template".into(),
        asset_id: "image".into(),
        content_type: "image/png".into(),
        bytes: vec![1, 2, 3],
    }];
    let repository = store_assets(document(&[]), &assets).await;
    for source in [
        "/import/other/assets/image",
        "/import/template/assets/missing",
    ] {
        let mut document = document(&[source]);
        assert_eq!(
            hydrate_canvas_assets(document.clone(), &assets)
                .unwrap_err()
                .code,
            "template_asset_not_found"
        );
        assert_eq!(
            hydrate_template_assets(&mut document, &repository)
                .await
                .unwrap_err()
                .code,
            "template_asset_not_found"
        );
    }
}

#[tokio::test]
async fn asset_hydration_preserves_inline_and_unrecognized_sources() {
    let original = document(&[
        "data:image/png;base64,AQID",
        "https://example.invalid/image.png",
        "/import/template/assets/image/extra",
    ]);
    let repository = store_assets(document(&[]), &[]).await;
    assert_eq!(
        hydrate_canvas_assets(original.clone(), &[]).unwrap(),
        original
    );
    let mut exported = original.clone();
    hydrate_template_assets(&mut exported, &repository)
        .await
        .unwrap();
    assert_eq!(exported, original);
}

#[tokio::test]
async fn asset_hydration_counts_each_reference_toward_the_embedded_image_limit() {
    let assets = vec![TemplateAsset {
        template_id: "template".into(),
        asset_id: "image".into(),
        content_type: "image/png".into(),
        bytes: vec![0; MAX_EMBEDDED_IMAGE_BYTES / 2 + 1],
    }];
    let stored = document(&[
        "/import/template/assets/image",
        "/import/template/assets/image",
    ]);
    let repository = store_assets(stored.clone(), &assets).await;
    assert_eq!(
        hydrate_canvas_assets(stored.clone(), &assets)
            .unwrap_err()
            .code,
        "template_too_large"
    );
    let mut exported = stored;
    assert_eq!(
        hydrate_template_assets(&mut exported, &repository)
            .await
            .unwrap_err()
            .code,
        "template_too_large"
    );
}
