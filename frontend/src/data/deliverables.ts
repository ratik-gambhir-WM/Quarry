import type { TemplatePreviewPage } from "@/contracts/quarryApi";

export const MAX_TEMPLATE_CATALOG_ITEMS = 1_000;
export const MAX_TEMPLATE_PREVIEW_PAGES = 100;

export type DeliverableSlide = {
  id: string;
  thumbnailAlt: string;
  thumbnailHeight: number;
  thumbnailSrc: string;
  thumbnailWidth: number;
};

export function templateDisplayName(templateId: string) {
  return templateId.replace(/[-_]+/g, " ").trim() || templateId;
}

export function previewToDeliverableSlide(
  preview: TemplatePreviewPage["previews"][number],
): DeliverableSlide {
  const displayName = templateDisplayName(preview.templateId);
  return {
    id: preview.templateId,
    thumbnailAlt: `Template preview: ${displayName}`,
    thumbnailHeight: preview.height,
    thumbnailSrc: preview.dataUrl,
    thumbnailWidth: preview.width,
  };
}
