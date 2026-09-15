import {
  useId,
  useMemo,
  useState,
} from 'react'

import type { DiligenceCanvasDocument } from '../../contracts/diligenceCanvas'
import { SvgSlideCanvas } from './internal/svg/SvgSlideCanvas'
import type { TextEditorOptions } from './internal/editor/textEditorConfig'

const CONTROL_CLASSES = [
  'min-h-[2.35rem] rounded-[0.45rem] border border-[#28304a] bg-[#080c1c]',
  'text-[0.78rem] [font-weight:750] text-[#eef3ff]',
  'focus-visible:border-[#f3c316] focus-visible:text-[#f3c316]',
  'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[rgba(243,195,22,0.35)]',
].join(' ')

const BUTTON_CLASSES = [
  CONTROL_CLASSES,
  'cursor-pointer px-3 py-[0.55rem] enabled:hover:border-[#f3c316]',
  'enabled:hover:text-[#f3c316] disabled:cursor-not-allowed disabled:opacity-50',
].join(' ')

export interface DiligenceCanvasProps {
  className?: string
  description?: string
  isJsonOpen?: boolean
  jsonPanelId?: string
  onChange: (document: DiligenceCanvasDocument) => void
  onJsonOpenChange?: (isOpen: boolean) => void
  onSlideIndexChange?: (slideIndex: number) => void
  resolveImageSource?: (source: string) => string
  showBranding?: boolean
  showJsonByDefault?: boolean
  showPresentationHeader?: boolean
  slideIndex?: number
  title?: string
  textEditorOptions?: TextEditorOptions
  value: DiligenceCanvasDocument
}

/**
 * A controlled, page-like PowerPoint template editor. The host owns `value`; every canvas edit
 * returns a complete replacement document through `onChange`.
 */
export function DiligenceCanvas({
  className,
  description = 'Inspect and edit this presentation in the current view.',
  isJsonOpen: controlledIsJsonOpen,
  jsonPanelId: controlledJsonPanelId,
  onChange,
  onJsonOpenChange,
  onSlideIndexChange,
  resolveImageSource,
  showBranding,
  showJsonByDefault = false,
  showPresentationHeader = true,
  slideIndex,
  title,
  textEditorOptions,
  value,
}: DiligenceCanvasProps) {
  const [internalSlideIndex, setInternalSlideIndex] = useState(0)
  const [internalIsJsonOpen, setInternalIsJsonOpen] = useState(showJsonByDefault)
  const generatedJsonPanelId = `${useId()}-json-panel`
  const isJsonOpen = controlledIsJsonOpen ?? internalIsJsonOpen
  const jsonPanelId = controlledJsonPanelId ?? generatedJsonPanelId
  const slideCount = value.presentation.slides.length
  const requestedSlideIndex = slideIndex ?? internalSlideIndex
  const activeSlideIndex = clampSlideIndex(requestedSlideIndex, slideCount)

  function updateJsonOpen(nextIsOpen: boolean) {
    if (controlledIsJsonOpen === undefined) {
      setInternalIsJsonOpen(nextIsOpen)
    }
    onJsonOpenChange?.(nextIsOpen)
  }

  function updateSlideIndex(nextIndex: number) {
    const clampedIndex = clampSlideIndex(nextIndex, slideCount)
    if (slideIndex === undefined) {
      setInternalSlideIndex(clampedIndex)
    }
    onSlideIndexChange?.(clampedIndex)
  }

  return (
    <section
      aria-label={showPresentationHeader ? undefined : title ?? value.presentation.title}
      className={[
        'diligence-canvas',
        'box-border flex w-full flex-col overflow-hidden font-sans',
        'motion-reduce:[&_*]:scroll-auto motion-reduce:[&_*]:duration-[0.01ms]',
        showPresentationHeader
          ? 'min-h-[42rem] gap-4 border border-white/10 bg-[#070a1b] text-[#eef3ff]'
          : 'min-h-0 bg-transparent text-foreground',
        className,
      ].filter(Boolean).join(' ')}
    >
      {showPresentationHeader ? (
        <header className="flex items-start justify-between gap-4 px-5 pt-5 max-[760px]:flex-col">
          <div className="min-w-0">
            <p className="m-0 text-[0.7rem] font-extrabold uppercase tracking-[0.16em] text-[#f3c316]">
              Diligence Canvas
            </p>
            <h1 className="mt-[0.3rem] mb-0 text-[clamp(1.55rem,3vw,2.4rem)] leading-[1.05] font-bold text-white">
              {title ?? value.presentation.title}
            </h1>
            <p className="mt-[0.55rem] mb-0 max-w-3xl text-[0.9rem] leading-[1.5] text-[#a8afc4]">
              {description}
            </p>
          </div>

          <div
            aria-label="Presentation actions"
            className="flex items-center gap-2 max-[760px]:flex-wrap"
            role="toolbar"
          >
            <button
              aria-controls={jsonPanelId}
              aria-expanded={isJsonOpen}
              className={BUTTON_CLASSES}
              onClick={() => updateJsonOpen(!isJsonOpen)}
              type="button"
            >
              {isJsonOpen ? 'Hide JSON' : 'Show JSON'}
            </button>
          </div>
        </header>
      ) : null}

      <div className={[
        'flex min-h-0 flex-1 max-[760px]:flex-col',
        showPresentationHeader ? 'border-t border-white/10' : '',
      ].filter(Boolean).join(' ')}>
        <div className="min-w-0 flex-1">
          <SvgSlideCanvas
            className={showPresentationHeader ? 'min-h-[32rem]' : 'min-h-0'}
            input={value}
            onChange={onChange}
            resolveImageSource={resolveImageSource}
            showBranding={showBranding}
            slideIndex={activeSlideIndex}
            textEditorOptions={textEditorOptions}
          />
        </div>

        {isJsonOpen ? (
          <PresentationJsonPanel
            id={jsonPanelId}
            onClose={() => updateJsonOpen(false)}
            value={value}
          />
        ) : null}
      </div>

      {slideCount > 1 ? (
        <footer className="flex items-center justify-center gap-2 px-5 pb-4 max-[760px]:flex-wrap">
          <button
            className={BUTTON_CLASSES}
            disabled={activeSlideIndex === 0}
            onClick={() => updateSlideIndex(activeSlideIndex - 1)}
            type="button"
          >
            Previous slide
          </button>
          <label className="flex items-center gap-2 text-xs font-bold text-[#a8afc4]">
            <span>Slide</span>
            <select
              className={`${CONTROL_CLASSES} max-w-[18rem] cursor-pointer py-[0.45rem] pr-8 pl-[0.65rem]`}
              onChange={(event) => updateSlideIndex(Number(event.currentTarget.value))}
              value={activeSlideIndex}
            >
              {value.presentation.slides.map((slide, index) => (
                <option key={slide.id} value={index}>
                  {index + 1}. {slide.name}
                </option>
              ))}
            </select>
          </label>
          <button
            className={BUTTON_CLASSES}
            disabled={activeSlideIndex >= slideCount - 1}
            onClick={() => updateSlideIndex(activeSlideIndex + 1)}
            type="button"
          >
            Next slide
          </button>
        </footer>
      ) : null}
    </section>
  )
}

function PresentationJsonPanel({
  id,
  onClose,
  value,
}: {
  id: string
  onClose: () => void
  value: DiligenceCanvasDocument
}) {
  const json = useMemo(() => JSON.stringify(value, null, 2), [value])

  return (
    <aside
      aria-label="Presentation JSON"
      className="flex w-[min(32rem,42%)] min-w-[19rem] flex-col border-l border-white/10 bg-[#0b0f24] max-[760px]:max-h-96 max-[760px]:w-full max-[760px]:min-w-0 max-[760px]:border-t max-[760px]:border-l-0"
      id={id}
    >
      <div className="flex items-center justify-between gap-2 px-4 py-[0.85rem]">
        <h2 className="m-0 text-[0.95rem] font-bold">Current JSON</h2>
        <button className={BUTTON_CLASSES} onClick={onClose} type="button">
          Close
        </button>
      </div>
      <pre className="m-0 min-h-0 flex-1 overflow-auto border-t border-[rgba(255,255,255,0.08)] bg-[#080c1c] p-4 font-mono text-[0.72rem] leading-[1.45] whitespace-pre text-[#d9e4ff]">
        {json}
      </pre>
    </aside>
  )
}

function clampSlideIndex(index: number, slideCount: number) {
  const safeIndex = Number.isFinite(index) ? Math.trunc(index) : 0
  return Math.max(0, Math.min(safeIndex, Math.max(slideCount - 1, 0)))
}
