//! Embedded built-in template catalog.

use super::{
    normalize, repository::png_dimensions, CanvasDocument, Element, StoredTemplate,
    StoredTemplateMetadata, StoredTemplatePreview, TemplateKind, TemplateRepository,
    TemplateSource,
};

const CATALOG_VERSION: i64 = 1;
const BUILTIN_CREATED_AT: i64 = 1_767_225_600_000;

struct CatalogEntry {
    template_id: &'static str,
    kind: TemplateKind,
    title: &'static str,
    description: &'static str,
    template_json: &'static str,
    preview: &'static [u8],
}

const ENTRIES: &[CatalogEntry] = &[
    CatalogEntry {
        template_id: "layered-architecture",
        kind: TemplateKind::Diagram,
        title: "Layered Architecture",
        description: "A clean layered view for showing presentation, service, integration, and data responsibilities across one application stack.",
        template_json: include_str!("catalog/templates/layered-arch.json"),
        preview: include_bytes!("catalog/previews/layered-arch.png"),
    },
    CatalogEntry {
        template_id: "layered-platform",
        kind: TemplateKind::Diagram,
        title: "Layered Platform",
        description: "A denser platform-oriented variant that helps explain shared services, internal enablement layers, and cross-cutting concerns.",
        template_json: include_str!("catalog/templates/layered-arch-2.json"),
        preview: include_bytes!("catalog/previews/layered-arch-2.png"),
    },
    CatalogEntry {
        template_id: "microservice-architecture",
        kind: TemplateKind::Diagram,
        title: "Microservice Architecture",
        description: "A microservice architecture template for showing service boundaries, API interaction points, data ownership, and supporting infrastructure across distributed capabilities.",
        template_json: include_str!("catalog/templates/microservice-arch.json"),
        preview: include_bytes!("catalog/previews/microservice-arch.png"),
    },
    CatalogEntry {
        template_id: "multi-tenant-system",
        kind: TemplateKind::Diagram,
        title: "Multi-Tenant System",
        description: "A multi-tenant diagram template for illustrating tenant isolation, shared infrastructure, and service boundaries in one product ecosystem.",
        template_json: include_str!("catalog/templates/two-system-arch.json"),
        preview: include_bytes!("catalog/previews/multi-tenant-arch.png"),
    },
    CatalogEntry {
        template_id: "multi-application-ecosystem",
        kind: TemplateKind::Diagram,
        title: "Multi-Application Ecosystem",
        description: "A multi-application architecture template for showing how several products or business applications interact through shared services, integrations, and data flows.",
        template_json: include_str!("catalog/templates/multi-app-arch.json"),
        preview: include_bytes!("catalog/previews/multi-app-arch.png"),
    },
    CatalogEntry {
        template_id: "product-architecture",
        kind: TemplateKind::Diagram,
        title: "Product Architecture",
        description: "A product-centric architecture view that emphasizes product modules, supporting integrations, and how business capabilities connect end to end.",
        template_json: include_str!("catalog/templates/multi-product-arch.json"),
        preview: include_bytes!("catalog/previews/product-arch-2.png"),
    },
    CatalogEntry {
        template_id: "architecture-ssa",
        kind: TemplateKind::Commentary,
        title: "Architecture SSA",
        description: "A commentary slide for summarizing architecture findings, impact, remediation themes, and recommended project work.",
        template_json: include_str!("catalog/templates/slide-01-phase-1.compact copy.json"),
        preview: include_bytes!("catalog/previews/arch-ssa.png"),
    },
    CatalogEntry {
        template_id: "security-ssa",
        kind: TemplateKind::Commentary,
        title: "Security SSA",
        description: "A commentary slide for secure development findings, remediation priorities, and phase-one project recommendations.",
        template_json: include_str!("catalog/templates/slide-02-phase-1.compact copy.json"),
        preview: include_bytes!("catalog/previews/security-ssa.png"),
    },
    CatalogEntry {
        template_id: "sdlc-ssa",
        kind: TemplateKind::Commentary,
        title: "SDLC SSA",
        description: "A commentary slide for SDLC and QA findings, operational execution gaps, and delivery improvement recommendations.",
        template_json: include_str!("catalog/templates/slide-03-phase-1.compact copy.json"),
        preview: include_bytes!("catalog/previews/sdlc-ssa.png"),
    },
];

pub(crate) fn seed(repository: &dyn TemplateRepository) -> anyhow::Result<()> {
    for entry in ENTRIES {
        let value = serde_json::from_str(entry.template_json)?;
        let (mut template_json, _) = normalize::normalize(&value)?;
        anyhow::ensure!(
            template_json.presentation.slides.len() == 1,
            "built-in template must contain exactly one slide"
        );
        if entry.kind == TemplateKind::Commentary {
            normalize_commentary(&mut template_json);
        }
        template_json.presentation.title = entry.title.to_owned();
        let mut asset_index = 0;
        let (template_json, assets) =
            super::service::externalize_assets_with(entry.template_id, template_json, || {
                asset_index += 1;
                format!("{}-asset-{asset_index}", entry.template_id)
            })?;
        let template = StoredTemplate {
            template_id: entry.template_id.to_owned(),
            template_json,
        };
        use sha2::{Digest, Sha256};
        let mut digest = Sha256::new();
        digest.update(CATALOG_VERSION.to_string());
        digest.update(entry.template_json);
        digest.update(entry.preview);
        digest.update(serde_json::to_string(&serde_json::json!({"description":entry.description,"json":format!("templates/{}.json",entry.template_id),"kind":entry.kind.as_str(),"preview":format!("previews/{}.png",entry.template_id),"templateId":entry.template_id,"title":entry.title}))?);
        let checksum = digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let metadata = StoredTemplateMetadata {
            template_id: entry.template_id.to_owned(),
            kind: entry.kind,
            title: entry.title.to_owned(),
            description: entry.description.to_owned(),
            source: TemplateSource::Builtin,
            created_at: BUILTIN_CREATED_AT,
            catalog_version: Some(CATALOG_VERSION),
            catalog_checksum: Some(checksum),
        };
        let (width, height) = png_dimensions(entry.preview)
            .ok_or_else(|| anyhow::anyhow!("built-in template preview is invalid"))?;
        let preview = StoredTemplatePreview {
            template_id: entry.template_id.to_owned(),
            content_type: "image/png".to_owned(),
            bytes: entry.preview.to_vec(),
            width,
            height,
        };
        repository.upsert_builtin(&template, &assets, &metadata, Some(&preview))?;
    }
    Ok(())
}

fn normalize_commentary(document: &mut CanvasDocument) {
    for slide in &mut document.presentation.slides {
        slide.elements.retain(|element| {
            let id = match element {
                Element::Image(value) => &value.id,
                Element::Text(value) => &value.id,
                Element::Shape(value) => &value.id,
                Element::Line(value) => &value.id,
            };
            if matches!(id.as_str(), "element-903000" | "west-monroe-footer" | "west-monroe-logo") || id.starts_with("west-monroe-dot-") {
                return false;
            }
            !matches!(element, Element::Image(image) if image.src.contains("element-903000.jpg") || image.src.contains("element-5.png"))
        });
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/templates/slide_templates/catalog_tests.rs"]
mod tests;
