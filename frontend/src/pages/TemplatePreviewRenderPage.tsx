import { SvgSlidePreview } from '@/lib/diligence-canvas/internal/svg/SvgSlidePreview'

const DEFAULT_MAX_DIMENSION = 1600
const MIN_MAX_DIMENSION = 320
const MAX_MAX_DIMENSION = 4096

export function TemplatePreviewRenderPage() {
  const input = window.__TTS_MERMAID_TEMPLATE_PREVIEW_INPUT__

  if (!input) {
    return <main data-preview-state="error">No template preview input was provided.</main>
  }

  return (
    <SvgSlidePreview
      input={input}
      maxDimension={readMaxDimension(window.location.search)}
    />
  )
}

function readMaxDimension(search: string) {
  const rawValue = new URLSearchParams(search).get('maxDimension')
  if (!rawValue || !/^\d+$/u.test(rawValue)) {
    return DEFAULT_MAX_DIMENSION
  }

  const value = Number(rawValue)
  return Number.isSafeInteger(value)
    ? Math.min(Math.max(value, MIN_MAX_DIMENSION), MAX_MAX_DIMENSION)
    : DEFAULT_MAX_DIMENSION
}
