import {
  useCallback,
  useEffect,
  useId,
  useMemo,
  useRef,
  useSyncExternalStore,
  type CSSProperties,
  type PointerEvent as ReactPointerEvent,
} from 'react'
import type { JsonValue } from '../shared/PowerpointTypes'
import type { TextEditorOptions } from '../editor/textEditorConfig'

import {
  applyElementEdit,
  buildSlideCanvasModel,
  type SlideElementRef,
} from '../canvas'
import { SvgSelection } from './SvgSelection'
import { SvgSlide } from './SvgSlide'
import { SvgSlideViewport } from './SvgSlideViewport'
import { SvgTextEditorOverlay } from './SvgTextEditorOverlay'
import {
  getConsistentSvgTextFontScales,
  getSvgTextScaleTemplateKey,
} from './svgTextLayout'
import { SUPPORTED_SHAPE_NAMES } from './shapes'
import { sanitizeSvgId, toSvgColor } from './svgUtils'
import { useSvgInteraction } from './useSvgInteraction'
import { useSvgViewport } from './useSvgViewport'

const VIEWPORT_BUTTON_CLASSES = [
  'min-w-9 cursor-pointer border-0 border-r border-white/20 bg-transparent',
  'px-[0.65rem] py-[0.45rem] text-xs font-bold text-white last:border-r-0',
  'hover:bg-[rgba(255,255,255,0.14)] focus-visible:bg-[rgba(255,255,255,0.14)] focus-visible:outline-none',
  'motion-reduce:transition-none',
].join(' ')

export interface SvgSlideCanvasProps<TInput extends JsonValue> {
  className?: string
  input: TInput
  onChange?: (input: TInput) => void
  resolveImageSource?: (source: string) => string
  showBranding?: boolean
  slideIndex?: number
  textEditorOptions?: TextEditorOptions
}

export function SvgSlideCanvas<TInput extends JsonValue>({
  className,
  input,
  onChange,
  resolveImageSource,
  showBranding,
  slideIndex = 0,
  textEditorOptions,
}: SvgSlideCanvasProps<TInput>) {
  const model = useMemo(
    () => buildSlideCanvasModel(input, { resolveImageSource, slideIndex }),
    [input, resolveImageSource, slideIndex],
  )
  const containerRef = useRef<HTMLDivElement>(null)
  const svgRef = useRef<SVGSVGElement>(null)
  const coordinateRootRef = useRef<SVGGElement>(null)
  const reactId = useId()
  const canvasId = useMemo(() => `svg-slide-${sanitizeSvgId(reactId)}`, [reactId])
  const viewportSize = useFittedViewportSize(containerRef, model.slide?.width, model.slide?.height)
  const viewport = useSvgViewport({
    slideHeight: model.slide?.height ?? 720,
    slideWidth: model.slide?.width ?? 1280,
    svgRef,
  })
  const interaction = useSvgInteraction({
    coordinateRootRef,
    elementRefs: model.elementRefs,
    input,
    onChange,
    svgRef,
  })
  const renderedRefs = useMemo(
    () =>
      interaction.state.draftEdits
        ? model.elementRefs.map((elementRef) =>
            interaction.state.draftEdits?.has(elementRef.key)
              ? {
                  ...elementRef,
                  element: applyElementEdit(
                    elementRef.element,
                    interaction.state.draftEdits.get(elementRef.key) ?? {},
                  ),
                }
              : elementRef,
          )
        : model.elementRefs,
    [interaction.state.draftEdits, model.elementRefs],
  )
  const renderedRefsByKey = useMemo(
    () => new Map(renderedRefs.map((elementRef) => [elementRef.key, elementRef])),
    [renderedRefs],
  )
  const textFontScales = useTemplateTextFontScales(model.elementRefs)
  const selectedRef = interaction.state.primaryKey
    ? renderedRefsByKey.get(interaction.state.primaryKey)
    : undefined
  const editingRef = interaction.state.editingKey
    ? renderedRefsByKey.get(interaction.state.editingKey)
    : undefined
  const editableTextElement =
    editingRef?.element.kind === 'text' || editingRef?.element.kind === 'shape'
      ? editingRef.element
      : undefined
  const selectedElementSupportsTextEditing =
    interaction.state.selectedKeys.size === 1 &&
    (selectedRef?.element.kind === 'text' || selectedRef?.element.kind === 'shape')
  const selectedRefs = renderedRefs.filter((elementRef) =>
    interaction.state.selectedKeys.has(elementRef.key),
  )
  const unsupportedShapes = useMemo(
    () =>
      model.elementRefs.filter(
        (elementRef) =>
          elementRef.element.kind === 'shape' &&
          !SUPPORTED_SHAPE_NAMES.has(elementRef.element.shape),
      ),
    [model.elementRefs],
  )

  useEffect(() => {
    if (import.meta.env.DEV && unsupportedShapes.length > 0) {
      console.warn(
        'Unsupported SVG slide shapes:',
        unsupportedShapes.map((elementRef) => ({
          key: elementRef.key,
          shape: elementRef.element.kind === 'shape' ? elementRef.element.shape : undefined,
        })),
      )
    }
  }, [unsupportedShapes])

  const handleElementPointerDown = useCallback(
    (elementRef: SlideElementRef, event: ReactPointerEvent<SVGElement>) => {
      if (!viewport.isPinching()) {
        interaction.beginDrag(elementRef, event)
      }
    },
    [interaction.beginDrag, viewport.isPinching],
  )
  const handleSlidePointerDown = useCallback(
    (event: ReactPointerEvent<SVGRectElement>) => {
      event.stopPropagation()
      if (event.shiftKey) {
        interaction.beginSelectionBox(event)
        return
      }
      interaction.selectElement()
      viewport.beginPan(event)
    },
    [interaction.beginSelectionBox, interaction.selectElement, viewport.beginPan],
  )
  const handlePointerMove = useCallback(
    (event: ReactPointerEvent<SVGSVGElement>) => {
      if (viewport.handlePointerMove(event)) {
        interaction.cancelActiveInteraction(false)
        return
      }
      interaction.handlePointerMove(event)
    },
    [interaction.cancelActiveInteraction, interaction.handlePointerMove, viewport.handlePointerMove],
  )
  const handlePointerUp = useCallback(
    (event: ReactPointerEvent<SVGSVGElement>) => {
      if (viewport.handlePointerEnd(event)) {
        interaction.cancelActiveInteraction(false)
        return
      }
      interaction.handlePointerUp(event)
    },
    [interaction.cancelActiveInteraction, interaction.handlePointerUp, viewport.handlePointerEnd],
  )
  const handlePointerCancel = useCallback(
    (event: ReactPointerEvent<SVGSVGElement>) => {
      viewport.handlePointerEnd(event)
      interaction.cancelActiveInteraction()
    },
    [interaction.cancelActiveInteraction, viewport.handlePointerEnd],
  )

  if (!model.slide) {
    return (
      <div
        className={[
          'grid min-h-[260px] place-items-center border border-dashed border-slate-900/20',
          'bg-slate-50 font-bold text-slate-500',
          className,
        ].filter(Boolean).join(' ')}
      >
        Unable to render this slide template.
      </div>
    )
  }

  const slide = model.slide
  const shouldShowBranding = showBranding ?? model.presentation?.meta.showBranding ?? true
  return (
    <div
      className={[
        'relative grid h-full min-h-[420px] w-full place-items-center overflow-hidden',
        'bg-[linear-gradient(90deg,rgba(20,29,54,0.05)_1px,transparent_1px),linear-gradient(0deg,rgba(20,29,54,0.05)_1px,transparent_1px),#eef2f8]',
        'bg-[size:32px_32px]',
        className,
      ].filter(Boolean).join(' ')}
      ref={containerRef}
      style={{ '--slide-canvas-bg': toSvgColor(slide.backgroundColor) } as CSSProperties}
    >
      <SvgSlideViewport
        clipId={`${canvasId}-root-clip`}
        contentRef={coordinateRootRef}
        contentTransform={viewport.transform}
        descriptionId={`${canvasId}-instructions`}
        id={canvasId}
        onKeyDown={interaction.handleKeyDown}
        onPointerCancel={handlePointerCancel}
        onPointerDownCapture={viewport.handlePointerDownCapture}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        onWheel={viewport.handleWheel}
        slide={slide}
        svgRef={svgRef}
        viewportSize={viewportSize}
      >
        <SvgSlide
          canvasId={canvasId}
          editingKey={interaction.state.editingKey}
          elementRefs={renderedRefs}
          onElementDoubleClick={interaction.startTextEditing}
          onElementFocus={interaction.focusElement}
          onElementPointerDown={handleElementPointerDown}
          onSlidePointerDown={handleSlidePointerDown}
          selectedKeys={interaction.state.selectedKeys}
          showBranding={shouldShowBranding}
          slide={slide}
          textFontScales={textFontScales}
        />
        {!interaction.state.editingKey
          ? selectedRefs.map((elementRef) => (
              <SvgSelection
                elementRef={elementRef}
                key={`selection-${elementRef.key}`}
                onLinePointPointerDown={interaction.beginLinePointMove}
                onResizePointerDown={interaction.beginResize}
                showHandles={interaction.state.selectedKeys.size === 1}
                zoom={viewport.state.zoom}
              />
            ))
          : null}
        {interaction.state.selectionBox ? (
          <rect
            className="fill-[rgba(20,81,225,0.1)] stroke-[rgba(20,81,225,0.72)] stroke-[1.5] [stroke-dasharray:5_4] [vector-effect:non-scaling-stroke]"
            height={interaction.state.selectionBox.h}
            pointerEvents="none"
            width={interaction.state.selectionBox.w}
            x={interaction.state.selectionBox.x}
            y={interaction.state.selectionBox.y}
          />
        ) : null}
      </SvgSlideViewport>

      <SvgViewportControls
        canvasId={canvasId}
        onReset={viewport.reset}
        onZoomIn={viewport.zoomIn}
        onZoomOut={viewport.zoomOut}
        zoom={viewport.state.zoom}
      />

      <p className="sr-only" id={`${canvasId}-instructions`}>
        Tab through slide elements. Press Space to select, Enter to edit text, arrow keys to
        move, Shift plus arrow keys to move farther, and Delete to remove. Shift-drag the
        slide background to select multiple elements.
      </p>

      <div aria-live="polite" className="sr-only" role="status">
        {interaction.announcement}
      </div>

      {selectedElementSupportsTextEditing && !interaction.state.editingKey ? (
        <div
          className="pointer-events-none absolute bottom-4 left-1/2 z-[4] max-w-[calc(100%-2rem)] -translate-x-1/2 rounded-lg border border-white/20 bg-slate-900/90 px-[0.7rem] py-[0.45rem] text-center text-xs leading-[1.2] font-semibold text-white shadow-[0_0.35rem_1rem_rgba(15,23,42,0.18)]"
          role="status"
        >
          Click again, double-click, or press Enter to edit text
        </div>
      ) : null}

      {editableTextElement ? (
        <SvgTextEditorOverlay
          containerRef={containerRef}
          coordinateRootRef={coordinateRootRef}
          element={editableTextElement}
          fontScale={
            interaction.state.editingKey
              ? textFontScales.get(interaction.state.editingKey)
              : undefined
          }
          key={interaction.state.editingKey}
          onCancel={interaction.cancelTextEditing}
          onCommit={interaction.commitText}
          textEditorOptions={textEditorOptions}
          viewportTransform={viewport.transform}
        />
      ) : null}

      {import.meta.env.DEV && unsupportedShapes.length > 0 ? (
        <div
          className="absolute bottom-4 left-4 border border-[rgba(255,0,80,0.55)] bg-[rgba(12,15,34,0.92)] px-[0.65rem] py-[0.45rem] text-xs font-bold text-[#ffb5c7]"
          role="status"
        >
          {unsupportedShapes.length} unsupported slide shape
          {unsupportedShapes.length === 1 ? '' : 's'}
        </div>
      ) : null}
    </div>
  )
}

function SvgViewportControls({
  canvasId,
  onReset,
  onZoomIn,
  onZoomOut,
  zoom,
}: {
  canvasId: string
  onReset: () => void
  onZoomIn: () => void
  onZoomOut: () => void
  zoom: number
}) {
  return (
    <div
      aria-label="Canvas zoom controls"
      className="absolute bottom-4 left-4 z-[6] flex overflow-hidden rounded-[0.65rem] border border-white/30 bg-slate-900/90 shadow-[0_0.35rem_1rem_rgba(15,23,42,0.2)]"
      role="toolbar"
    >
      <button
        aria-controls={canvasId}
        aria-label="Zoom out"
        className={VIEWPORT_BUTTON_CLASSES}
        onClick={onZoomOut}
        type="button"
      >
        −
      </button>
      <button
        aria-controls={canvasId}
        className={VIEWPORT_BUTTON_CLASSES}
        onClick={onReset}
        type="button"
      >
        Fit {Math.round(zoom * 100)}%
      </button>
      <button
        aria-controls={canvasId}
        aria-label="Zoom in"
        className={VIEWPORT_BUTTON_CLASSES}
        onClick={onZoomIn}
        type="button"
      >
        +
      </button>
    </div>
  )
}

function useTemplateTextFontScales(elementRefs: ReturnType<typeof buildSlideCanvasModel>['elementRefs']) {
  const templateKey = getSvgTextScaleTemplateKey(elementRefs)
  const cachedScalesRef = useRef<{
    fontScales: Map<string, number>
    templateKey: string
  } | undefined>(undefined)

  if (!cachedScalesRef.current || cachedScalesRef.current.templateKey !== templateKey) {
    cachedScalesRef.current = {
      fontScales: getConsistentSvgTextFontScales(elementRefs),
      templateKey,
    }
  }

  return cachedScalesRef.current.fontScales
}

function useFittedViewportSize(
  containerRef: React.RefObject<HTMLDivElement | null>,
  slideWidth?: number,
  slideHeight?: number,
) {
  const getSnapshot = useCallback(() => {
    const container = containerRef.current
    if (!container || !slideWidth || !slideHeight) {
      return ''
    }

    const availableWidth = Math.max(container.clientWidth - 32, 0)
    const availableHeight = Math.max(container.clientHeight - 32, 0)
    const scale = Math.min(availableWidth / slideWidth, availableHeight / slideHeight)
    return `${slideWidth * scale}:${slideHeight * scale}`
  }, [containerRef, slideHeight, slideWidth])
  const subscribe = useCallback((notify: () => void) => {
    const container = containerRef.current
    if (!container) return () => undefined

    const observer = new ResizeObserver(notify)
    observer.observe(container)
    return () => observer.disconnect()
  }, [containerRef])
  const snapshot = useSyncExternalStore(subscribe, getSnapshot, () => '')

  if (!snapshot) return undefined
  const [width, height] = snapshot.split(':').map(Number)

  return { height, width }
}
