import {
  lazy,
  Suspense,
  useCallback,
  useId,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type CSSProperties,
  type RefObject,
} from 'react'

import type { PlateTextEditorHandle } from '../editor/PlateTextEditor'
import type { RichTextEdit } from '../editor/plateValue'
import type { TextEditorOptions } from '../editor/textEditorConfig'
import type {
  NormalizedShapeElement,
  NormalizedTextElement,
} from '../shared/PowerpointTypes'
import { SvgTextBlock } from './SvgText'
import { getSvgTextContent } from './svgTextLayout'
import { getTextEditorContentScale } from './textEditorLayout'
import { sanitizeSvgId, toSvgColor } from './svgUtils'

type EditableTextElement = NormalizedShapeElement | NormalizedTextElement

const PlateTextEditor = lazy(() =>
  import('../editor/PlateTextEditor').then((module) => ({
    default: module.PlateTextEditor,
  })),
)

type OverlayPosition = {
  height: number
  left: number
  scale: number
  top: number
  width: number
}

export function SvgTextEditorOverlay({
  containerRef,
  coordinateRootRef,
  element,
  fontScale,
  onCancel,
  onCommit,
  textEditorOptions,
  viewportTransform,
}: {
  containerRef: RefObject<HTMLDivElement | null>
  coordinateRootRef: RefObject<SVGGraphicsElement | null>
  element: EditableTextElement
  fontScale?: number
  onCancel: () => void
  onCommit: (edit: RichTextEdit) => void
  textEditorOptions?: TextEditorOptions
  viewportTransform: string
}) {
  const textContent = useMemo(
    () => getSvgTextContent(element, fontScale),
    [element, fontScale],
  )
  const editorRef = useRef<PlateTextEditorHandle>(null)
  const completedRef = useRef(false)
  const snapshotClipId = `text-editor-${sanitizeSvgId(useId())}-snapshot-clip`
  const [content, setContent] = useState<HTMLDivElement | null>(null)
  const [hasDraftChange, setHasDraftChange] = useState(false)
  const position = useOverlayPosition({
    containerRef,
    coordinateRootRef,
    element,
    viewportTransform,
  })
  const [isOverflowing, refreshOverflow] = useElementOverflow(content)

  if (!position) return null

  const editorStyle: CSSProperties = {
    height: position.height,
    left: position.left,
    top: position.top,
    transform: element.rotate ? `rotate(${element.rotate}deg)` : undefined,
    width: position.width,
  }
  const effectiveContentScale = getTextEditorContentScale(textContent.layout.fontScale)
  const caretColor = toSvgColor(textContent.runs[0]?.color ?? textContent.fallbackRun.color)
  const firstLine = textContent.layout.lines[0]
  const firstRunTop = firstLine
    ? firstLine.baseline - firstLine.height * 0.82
    : element.padding
  const contentStyle: CSSProperties = {
    height: element.h / effectiveContentScale,
    paddingLeft: element.padding / effectiveContentScale,
    paddingRight: element.padding / effectiveContentScale,
    paddingTop: firstRunTop / effectiveContentScale,
    textAlign: element.align,
    transform: `scale(${position.scale * effectiveContentScale})`,
    width: element.w / effectiveContentScale,
  }
  const commitDraft = () => {
    if (completedRef.current) return
    const edit = editorRef.current?.getDraft()
    if (!edit) return
    completedRef.current = true
    onCommit(edit)
  }

  return (
    <div
      className={[
        'absolute z-[5] box-border h-full w-full min-w-0 origin-center overflow-visible',
        'm-0 rounded-[2px] border-0 bg-transparent outline-none',
        isOverflowing
          ? 'shadow-[inset_0_0_0_2px_#dc2626]'
          : 'shadow-[inset_0_0_0_2px_rgba(20,81,225,0.9)]',
      ].join(' ')}
      data-overflow={isOverflowing || undefined}
      onBlur={(event) => {
        const nextTarget = event.relatedTarget
        if (nextTarget instanceof Node && event.currentTarget.contains(nextTarget)) return
        commitDraft()
      }}
      onKeyDownCapture={(event) => {
        event.stopPropagation()
        if (event.key === 'Escape') {
          event.preventDefault()
          completedRef.current = true
          onCancel()
        } else if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
          event.preventDefault()
          commitDraft()
        }
      }}
      onPointerDown={(event) => event.stopPropagation()}
      style={editorStyle}
    >
      {!hasDraftChange ? (
        <svg
          aria-hidden="true"
          className="pointer-events-none absolute inset-0 h-full w-full overflow-visible"
          preserveAspectRatio="none"
          viewBox={`0 0 ${element.w} ${element.h}`}
        >
          <SvgTextBlock
            clipId={snapshotClipId}
            element={element}
            fontScale={fontScale}
          />
        </svg>
      ) : null}
      <Suspense
        fallback={(
          <span className="sr-only" role="status">
            Loading text editor
          </span>
        )}
      >
        <PlateTextEditor
          caretColor={caretColor}
          contentRef={setContent}
          contentStyle={contentStyle}
          element={element}
          hideText={!hasDraftChange}
          onDraftChange={() => {
            setHasDraftChange(true)
            requestAnimationFrame(refreshOverflow)
          }}
          options={textEditorOptions}
          ref={editorRef}
        />
      </Suspense>
      {isOverflowing ? (
        <span
          className="absolute top-[calc(100%+0.35rem)] right-0 rounded-[0.35rem] bg-red-800 px-[0.4rem] py-1 font-sans text-[0.68rem] leading-[1.2] font-semibold whitespace-nowrap text-white"
          role="status"
        >
          Text exceeds shape bounds
        </span>
      ) : null}
    </div>
  )
}

function useOverlayPosition({
  containerRef,
  coordinateRootRef,
  element,
  viewportTransform,
}: {
  containerRef: RefObject<HTMLDivElement | null>
  coordinateRootRef: RefObject<SVGGraphicsElement | null>
  element: EditableTextElement
  viewportTransform: string
}) {
  const getSnapshot = useCallback(() => {
    const coordinateRoot = coordinateRootRef.current
    const container = containerRef.current
    const matrix = coordinateRoot?.getScreenCTM()
    if (!coordinateRoot || !container || !matrix) return ''

    const containerRect = container.getBoundingClientRect()
    const topLeft = new DOMPoint(element.x, element.y).matrixTransform(matrix)
    const scale = Math.hypot(matrix.a, matrix.b)
    return JSON.stringify({
      height: element.h * scale,
      left: topLeft.x - containerRect.left,
      scale,
      top: topLeft.y - containerRect.top,
      width: element.w * scale,
    } satisfies OverlayPosition)
  }, [containerRef, coordinateRootRef, element.h, element.w, element.x, element.y, viewportTransform])
  const subscribe = useCallback((notify: () => void) => {
    const coordinateRoot = coordinateRootRef.current
    const container = containerRef.current
    if (!coordinateRoot || !container) return () => undefined

    const observer = new ResizeObserver(notify)
    observer.observe(container)
    observer.observe(coordinateRoot)
    window.addEventListener('resize', notify)
    return () => {
      observer.disconnect()
      window.removeEventListener('resize', notify)
    }
  }, [containerRef, coordinateRootRef])
  const snapshot = useSyncExternalStore(subscribe, getSnapshot, () => '')

  return useMemo(
    () => snapshot ? JSON.parse(snapshot) as OverlayPosition : undefined,
    [snapshot],
  )
}

function useElementOverflow(content: HTMLDivElement | null): [boolean, () => void] {
  const listenersRef = useRef(new Set<() => void>())
  const getSnapshot = useCallback(
    () => Boolean(
      content && (
        content.scrollHeight > content.clientHeight + 1 ||
        content.scrollWidth > content.clientWidth + 1
      )
    ),
    [content],
  )
  const subscribe = useCallback((notify: () => void) => {
    if (!content) return () => undefined

    listenersRef.current.add(notify)
    const observer = new ResizeObserver(notify)
    observer.observe(content)
    return () => {
      listenersRef.current.delete(notify)
      observer.disconnect()
    }
  }, [content])
  const refresh = useCallback(() => {
    listenersRef.current.forEach((notify) => notify())
  }, [])

  return [useSyncExternalStore(subscribe, getSnapshot, () => false), refresh]
}
