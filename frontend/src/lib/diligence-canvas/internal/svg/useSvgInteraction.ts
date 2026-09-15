import {
  useCallback,
  useMemo,
  useReducer,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type RefObject,
} from 'react'

import type { JsonValue, NormalizedElement } from '../shared/PowerpointTypes'
import {
  fromPlateValue,
  toPlateValue,
  type RichTextEdit,
} from '../editor/plateValue'
import {
  applyElementEditsToInput,
  deleteElementsFromInput,
  getElementAccessibleLabel,
  getElementGeometry,
  getRenderedLinePoints,
  roundCoordinate,
  type ElementEdit,
  type SlideElementRef,
} from '../canvas'

export type ResizeHandle = 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w' | 'nw'

export type SelectionBox = {
  h: number
  w: number
  x: number
  y: number
}

export type SvgInteractionState = {
  draftEdits?: ReadonlyMap<string, ElementEdit>
  editingKey?: string
  mode: 'idle' | 'dragging' | 'resizing' | 'editing-text' | 'moving-line-point' | 'selecting'
  primaryKey?: string
  selectedKeys: ReadonlySet<string>
  selectionBox?: SelectionBox
}

type InteractionAction =
  | {
      mode: ActiveInteraction['mode']
      primaryKey: string
      selectedKeys: ReadonlySet<string>
      type: 'begin'
    }
  | { type: 'cancel' }
  | { edits: ReadonlyMap<string, ElementEdit>; mode: ActiveInteraction['mode']; type: 'draft' }
  | { key: string; type: 'edit-text' }
  | { box: SelectionBox; type: 'selection-box' }
  | { primaryKey?: string; selectedKeys: ReadonlySet<string>; type: 'select' }
  | { type: 'settle' }

type ActiveInteraction = {
  didMove: boolean
  handle?: ResizeHandle | 'line-end' | 'line-start'
  lastEdits?: ReadonlyMap<string, ElementEdit>
  mode: 'dragging' | 'resizing' | 'moving-line-point'
  openTextEditorOnClick: boolean
  pointerId: number
  primaryRef: SlideElementRef
  refs: SlideElementRef[]
  startElements: ReadonlyMap<string, NormalizedElement>
  startPoint: { x: number; y: number }
}

type ActiveSelectionBox = {
  baseKeys: ReadonlySet<string>
  pointerId: number
  startPoint: { x: number; y: number }
}

type UseSvgInteractionOptions<TInput extends JsonValue> = {
  coordinateRootRef: RefObject<SVGGraphicsElement | null>
  elementRefs: SlideElementRef[]
  input: TInput
  onChange?: (input: TInput) => void
  svgRef: RefObject<SVGSVGElement | null>
}

const INITIAL_STATE: SvgInteractionState = {
  mode: 'idle',
  selectedKeys: new Set(),
}

export function useSvgInteraction<TInput extends JsonValue>({
  coordinateRootRef,
  elementRefs,
  input,
  onChange,
  svgRef,
}: UseSvgInteractionOptions<TInput>) {
  const [state, dispatch] = useReducer(interactionReducer, INITIAL_STATE)
  const [announcement, setAnnouncement] = useState('')
  const activeRef = useRef<ActiveInteraction | undefined>(undefined)
  const selectionBoxRef = useRef<ActiveSelectionBox | undefined>(undefined)
  const latestInputRef = useRef(input)
  const latestOnChangeRef = useRef(onChange)
  latestInputRef.current = input
  latestOnChangeRef.current = onChange
  const refsByKey = useMemo(
    () => new Map(elementRefs.map((elementRef) => [elementRef.key, elementRef])),
    [elementRefs],
  )

  const currentState = useMemo(() => {
    const selectedKeys = new Set(
      Array.from(state.selectedKeys).filter((key) => refsByKey.has(key)),
    )
    if (selectedKeys.size === state.selectedKeys.size) {
      return state
    }

    return {
      ...state,
      primaryKey:
        state.primaryKey && selectedKeys.has(state.primaryKey)
          ? state.primaryKey
          : selectedKeys.values().next().value,
      selectedKeys,
    }
  }, [refsByKey, state])

  const selectElement = useCallback((key?: string) => {
    dispatch({
      primaryKey: key,
      selectedKeys: key ? new Set([key]) : new Set(),
      type: 'select',
    })
  }, [])

  const focusElement = useCallback((ref: SlideElementRef) => {
    dispatch({
      primaryKey: ref.key,
      selectedKeys: new Set([ref.key]),
      type: 'select',
    })
  }, [])

  const begin = useCallback(
    (
      ref: SlideElementRef,
      event: ReactPointerEvent<SVGElement>,
      mode: ActiveInteraction['mode'],
      handle?: ActiveInteraction['handle'],
    ) => {
      if (event.button !== 0 || currentState.editingKey) {
        return
      }

      const svg = svgRef.current
      const coordinateRoot = coordinateRootRef.current
      const startPoint = coordinateRoot
        ? clientPointToSlide(coordinateRoot, event.clientX, event.clientY)
        : undefined
      if (!svg || !startPoint) {
        return
      }

      const selectedKeys = new Set(currentState.selectedKeys)
      if (mode === 'dragging' && event.shiftKey) {
        if (selectedKeys.has(ref.key)) {
          selectedKeys.delete(ref.key)
          dispatch({
            primaryKey: selectedKeys.values().next().value,
            selectedKeys,
            type: 'select',
          })
          return
        }
        selectedKeys.add(ref.key)
      } else if (mode !== 'dragging' || !selectedKeys.has(ref.key)) {
        selectedKeys.clear()
        selectedKeys.add(ref.key)
      }

      const refs = mode === 'dragging'
        ? elementRefs.filter((elementRef) => selectedKeys.has(elementRef.key))
        : [ref]

      event.preventDefault()
      event.stopPropagation()
      svg.focus({ preventScroll: true })
      svg.setPointerCapture(event.pointerId)
      activeRef.current = {
        didMove: false,
        handle,
        mode,
        openTextEditorOnClick:
          mode === 'dragging' &&
          !event.shiftKey &&
          currentState.selectedKeys.size === 1 &&
          currentState.selectedKeys.has(ref.key) &&
          (ref.element.kind === 'shape' || ref.element.kind === 'text'),
        pointerId: event.pointerId,
        primaryRef: ref,
        refs,
        startElements: new Map(
          refs.map((elementRef) => [elementRef.key, elementRef.element]),
        ),
        startPoint,
      }
      dispatch({ mode, primaryKey: ref.key, selectedKeys, type: 'begin' })
    },
    [coordinateRootRef, currentState.editingKey, currentState.selectedKeys, elementRefs, svgRef],
  )

  const beginDrag = useCallback(
    (ref: SlideElementRef, event: ReactPointerEvent<SVGElement>) =>
      begin(ref, event, 'dragging'),
    [begin],
  )

  const beginResize = useCallback(
    (
      ref: SlideElementRef,
      handle: ResizeHandle,
      event: ReactPointerEvent<SVGElement>,
    ) => begin(ref, event, 'resizing', handle),
    [begin],
  )

  const beginLinePointMove = useCallback(
    (
      ref: SlideElementRef,
      point: 'end' | 'start',
      event: ReactPointerEvent<SVGElement>,
    ) => begin(ref, event, 'moving-line-point', point === 'start' ? 'line-start' : 'line-end'),
    [begin],
  )

  const beginSelectionBox = useCallback(
    (event: ReactPointerEvent<SVGElement>) => {
      const svg = svgRef.current
      const coordinateRoot = coordinateRootRef.current
      const startPoint = coordinateRoot
        ? clientPointToSlide(coordinateRoot, event.clientX, event.clientY)
        : undefined
      if (event.button !== 0 || !svg || !startPoint || currentState.editingKey) {
        return
      }

      event.preventDefault()
      event.stopPropagation()
      svg.focus({ preventScroll: true })
      svg.setPointerCapture(event.pointerId)
      selectionBoxRef.current = {
        baseKeys: currentState.selectedKeys,
        pointerId: event.pointerId,
        startPoint,
      }
      dispatch({ box: { h: 0, w: 0, x: startPoint.x, y: startPoint.y }, type: 'selection-box' })
    },
    [coordinateRootRef, currentState.editingKey, currentState.selectedKeys, svgRef],
  )

  const handlePointerMove = useCallback(
    (event: ReactPointerEvent<SVGSVGElement>) => {
      const coordinateRoot = coordinateRootRef.current
      const selectionBox = selectionBoxRef.current
      if (selectionBox && coordinateRoot && event.pointerId === selectionBox.pointerId) {
        const point = clientPointToSlide(coordinateRoot, event.clientX, event.clientY)
        if (point) {
          dispatch({ box: makeSelectionBox(selectionBox.startPoint, point), type: 'selection-box' })
        }
        return
      }

      const active = activeRef.current
      if (!active || !coordinateRoot || event.pointerId !== active.pointerId) {
        return
      }

      const point = clientPointToSlide(coordinateRoot, event.clientX, event.clientY)
      if (!point) {
        return
      }

      const dx = point.x - active.startPoint.x
      const dy = point.y - active.startPoint.y
      if (active.mode === 'dragging' && !active.didMove && Math.hypot(dx, dy) < 3) {
        return
      }

      active.didMove = true
      const edits = new Map<string, ElementEdit>()
      active.refs.forEach((elementRef) => {
        const startElement = active.startElements.get(elementRef.key)
        if (startElement) {
          edits.set(elementRef.key, getInteractionEdit(active, startElement, dx, dy))
        }
      })
      active.lastEdits = edits
      dispatch({ edits, mode: active.mode, type: 'draft' })
    },
    [coordinateRootRef],
  )

  const releasePointer = useCallback(
    (pointerId: number) => {
      if (svgRef.current?.hasPointerCapture(pointerId)) {
        svgRef.current.releasePointerCapture(pointerId)
      }
    },
    [svgRef],
  )

  const handlePointerUp = useCallback(
    (event: ReactPointerEvent<SVGSVGElement>) => {
      const activeSelectionBox = selectionBoxRef.current
      if (activeSelectionBox && event.pointerId === activeSelectionBox.pointerId) {
        releasePointer(event.pointerId)
        selectionBoxRef.current = undefined
        const selectedKeys = new Set(activeSelectionBox.baseKeys)
        const box = currentState.selectionBox
        if (box) {
          elementRefs.forEach((elementRef) => {
            if (boxesIntersect(box, getElementGeometry(elementRef.element))) {
              selectedKeys.add(elementRef.key)
            }
          })
        }
        dispatch({
          primaryKey: Array.from(selectedKeys).slice(-1)[0],
          selectedKeys,
          type: 'select',
        })
        return
      }

      const active = activeRef.current
      if (!active || event.pointerId !== active.pointerId) {
        return
      }

      releasePointer(event.pointerId)
      activeRef.current = undefined
      if (active.openTextEditorOnClick && !active.didMove && !active.lastEdits) {
        dispatch({ key: active.primaryRef.key, type: 'edit-text' })
        return
      }

      if (active.lastEdits && latestOnChangeRef.current) {
        latestOnChangeRef.current(
          applyElementEditsToInput(
            latestInputRef.current,
            active.refs.flatMap((locator) => {
              const edit = active.lastEdits?.get(locator.key)
              return edit ? [{ edit, locator }] : []
            }),
          ),
        )
        setAnnouncement(
          active.refs.length === 1
            ? describeElementPosition(active.primaryRef, active.lastEdits.get(active.primaryRef.key))
            : `${active.refs.length} elements moved.`,
        )
      }
      dispatch({ type: 'settle' })
    },
    [currentState.selectionBox, elementRefs, releasePointer],
  )

  const cancelActiveInteraction = useCallback((shouldReleasePointer = true) => {
    if (!activeRef.current && !selectionBoxRef.current) {
      return
    }
    const pointerId = activeRef.current?.pointerId ?? selectionBoxRef.current?.pointerId
    if (shouldReleasePointer && pointerId !== undefined) {
      releasePointer(pointerId)
    }
    activeRef.current = undefined
    selectionBoxRef.current = undefined
    dispatch({ type: 'settle' })
  }, [releasePointer])

  const startTextEditing = useCallback((ref: SlideElementRef) => {
    if (ref.element.kind === 'shape' || ref.element.kind === 'text') {
      dispatch({ key: ref.key, type: 'edit-text' })
    }
  }, [])

  const commitText = useCallback(
    (edit: RichTextEdit) => {
      const ref = currentState.editingKey ? refsByKey.get(currentState.editingKey) : undefined
      if (ref && latestOnChangeRef.current) {
        const currentEdit = ref.element.kind === 'text' || ref.element.kind === 'shape'
          ? fromPlateValue(toPlateValue(ref.element), ref.element)
          : undefined
        if (currentEdit && !sameRichTextEdit(edit, currentEdit)) {
          latestOnChangeRef.current(
            applyElementEditsToInput(latestInputRef.current, [
              { edit, locator: ref },
            ]),
          )
          setAnnouncement(`${getElementAccessibleLabel(ref.element)} text updated.`)
        }
      }
      dispatch({ type: 'settle' })
    },
    [currentState.editingKey, refsByKey],
  )

  const cancelTextEditing = useCallback(() => {
    dispatch({ type: 'settle' })
  }, [])

  const handleKeyDown = useCallback(
    (event: ReactKeyboardEvent<SVGSVGElement>) => {
      if (currentState.editingKey) {
        return
      }

      if (event.key === 'Escape') {
        event.preventDefault()
        const pointerId = activeRef.current?.pointerId ?? selectionBoxRef.current?.pointerId
        if (pointerId !== undefined) {
          releasePointer(pointerId)
        }
        activeRef.current = undefined
        selectionBoxRef.current = undefined
        dispatch({ type: 'cancel' })
        setAnnouncement('Selection cleared.')
        return
      }

      const focusedKey = getFocusedElementKey(event.target)
      if (event.key === ' ' && focusedKey) {
        event.preventDefault()
        const ref = refsByKey.get(focusedKey)
        if (ref) {
          focusElement(ref)
          setAnnouncement(`${getElementAccessibleLabel(ref.element)} selected.`)
        }
        return
      }

      const selectedRefs = elementRefs.filter((elementRef) =>
        currentState.selectedKeys.has(elementRef.key),
      )
      if (selectedRefs.length === 0) {
        return
      }

      if (event.key === 'Enter' && selectedRefs.length === 1) {
        const [ref] = selectedRefs
        if (ref.element.kind === 'shape' || ref.element.kind === 'text') {
          event.preventDefault()
          dispatch({ key: ref.key, type: 'edit-text' })
        }
        return
      }

      if (event.key === 'Delete' || event.key === 'Backspace') {
        event.preventDefault()
        latestOnChangeRef.current?.(
          deleteElementsFromInput(latestInputRef.current, selectedRefs),
        )
        setAnnouncement(
          selectedRefs.length === 1
            ? `${getElementAccessibleLabel(selectedRefs[0].element)} deleted.`
            : `${selectedRefs.length} elements deleted.`,
        )
        dispatch({ primaryKey: undefined, selectedKeys: new Set(), type: 'select' })
        svgRef.current?.focus({ preventScroll: true })
        return
      }

      const distance = event.shiftKey ? 10 : 1
      const delta = getArrowDelta(event.key, distance)
      if (!delta) {
        return
      }

      event.preventDefault()
      const requests = selectedRefs.map((locator) => ({
        edit: getNudgeEdit(locator.element, delta),
        locator,
      }))
      latestOnChangeRef.current?.(
        applyElementEditsToInput(latestInputRef.current, requests),
      )
      setAnnouncement(
        selectedRefs.length === 1
          ? describeElementPosition(selectedRefs[0], requests[0].edit)
          : `${selectedRefs.length} elements moved ${distance} slide units ${getDirection(event.key)}.`,
      )
    },
    [currentState.editingKey, currentState.selectedKeys, elementRefs, focusElement, refsByKey, releasePointer, svgRef],
  )

  return {
    announcement,
    beginDrag,
    beginLinePointMove,
    beginResize,
    beginSelectionBox,
    cancelActiveInteraction,
    cancelTextEditing,
    commitText,
    focusElement,
    handleKeyDown,
    handlePointerMove,
    handlePointerUp,
    selectElement,
    startTextEditing,
    state: currentState,
  }
}

function sameRichTextEdit(left: RichTextEdit, right: RichTextEdit) {
  return left.text === right.text &&
    left.align === right.align &&
    JSON.stringify(left.runs) === JSON.stringify(right.runs)
}

function interactionReducer(
  state: SvgInteractionState,
  action: InteractionAction,
): SvgInteractionState {
  switch (action.type) {
    case 'select':
      return {
        mode: 'idle',
        primaryKey: action.primaryKey,
        selectedKeys: action.selectedKeys,
      }
    case 'begin':
      return {
        mode: action.mode,
        primaryKey: action.primaryKey,
        selectedKeys: action.selectedKeys,
      }
    case 'draft':
      return {
        ...state,
        draftEdits: action.edits,
        mode: action.mode,
      }
    case 'selection-box':
      return {
        ...state,
        mode: 'selecting',
        selectionBox: action.box,
      }
    case 'edit-text':
      return {
        editingKey: action.key,
        mode: 'editing-text',
        primaryKey: action.key,
        selectedKeys: new Set([action.key]),
      }
    case 'cancel':
      return {
        mode: 'idle',
        selectedKeys: new Set(),
      }
    case 'settle':
      return {
        mode: 'idle',
        primaryKey: state.primaryKey,
        selectedKeys: state.selectedKeys,
      }
  }
}

function clientPointToSlide(
  coordinateRoot: SVGGraphicsElement,
  clientX: number,
  clientY: number,
) {
  const matrix = coordinateRoot.getScreenCTM()?.inverse()
  if (!matrix) {
    return undefined
  }

  const point = new DOMPoint(clientX, clientY).matrixTransform(matrix)
  return { x: point.x, y: point.y }
}

function getInteractionEdit(
  active: ActiveInteraction,
  element: NormalizedElement,
  dx: number,
  dy: number,
): ElementEdit {
  if (active.mode === 'dragging') {
    if (element.kind === 'line') {
      return {
        x1: roundCoordinate(element.x1 + dx),
        x2: roundCoordinate(element.x2 + dx),
        y1: roundCoordinate(element.y1 + dy),
        y2: roundCoordinate(element.y2 + dy),
      }
    }
    return {
      x: roundCoordinate(element.x + dx),
      y: roundCoordinate(element.y + dy),
    }
  }

  if (active.mode === 'moving-line-point' && element.kind === 'line') {
    return getLinePointEdit(element, active.handle === 'line-start' ? 'start' : 'end', dx, dy)
  }

  if (element.kind === 'line' || !active.handle) {
    return {}
  }

  return getResizeEdit(element, active.handle as ResizeHandle, dx, dy)
}

function getLinePointEdit(
  element: Extract<NormalizedElement, { kind: 'line' }>,
  point: 'end' | 'start',
  dx: number,
  dy: number,
): ElementEdit {
  if (!element.rotate) {
    return point === 'start'
      ? { x1: roundCoordinate(element.x1 + dx), y1: roundCoordinate(element.y1 + dy) }
      : { x2: roundCoordinate(element.x2 + dx), y2: roundCoordinate(element.y2 + dy) }
  }

  const rendered = getRenderedLinePoints(element)
  const start = {
    x: rendered.x1 + (point === 'start' ? dx : 0),
    y: rendered.y1 + (point === 'start' ? dy : 0),
  }
  const end = {
    x: rendered.x2 + (point === 'end' ? dx : 0),
    y: rendered.y2 + (point === 'end' ? dy : 0),
  }
  const center = {
    x: (start.x + end.x) / 2,
    y: (start.y + end.y) / 2,
  }
  const rawStart = rotatePoint(start, center, -element.rotate)
  const rawEnd = rotatePoint(end, center, -element.rotate)

  return {
    x1: roundCoordinate(rawStart.x),
    x2: roundCoordinate(rawEnd.x),
    y1: roundCoordinate(rawStart.y),
    y2: roundCoordinate(rawEnd.y),
  }
}

function rotatePoint(
  point: { x: number; y: number },
  center: { x: number; y: number },
  degrees: number,
) {
  const radians = (degrees * Math.PI) / 180
  const cosine = Math.cos(radians)
  const sine = Math.sin(radians)
  const x = point.x - center.x
  const y = point.y - center.y
  return {
    x: center.x + x * cosine - y * sine,
    y: center.y + x * sine + y * cosine,
  }
}

function getResizeEdit(
  element: Exclude<NormalizedElement, { kind: 'line' }>,
  handle: ResizeHandle,
  dx: number,
  dy: number,
) {
  const localDelta = rotateVector({ x: dx, y: dy }, -element.rotate)
  const movesWest = handle.includes('w')
  const movesEast = handle.includes('e')
  const movesNorth = handle.includes('n')
  const movesSouth = handle.includes('s')
  let w = element.w
  let h = element.h

  if (movesWest) {
    w = Math.max(36, element.w - localDelta.x)
  } else if (movesEast) {
    w = Math.max(36, element.w + localDelta.x)
  }

  if (movesNorth) {
    h = Math.max(24, element.h - localDelta.y)
  } else if (movesSouth) {
    h = Math.max(24, element.h + localDelta.y)
  }

  const leftOffset = movesWest ? element.w - w : 0
  const topOffset = movesNorth ? element.h - h : 0
  const oldCenter = { x: element.w / 2, y: element.h / 2 }
  const resizedCenter = rotatePoint(
    { x: leftOffset + w / 2, y: topOffset + h / 2 },
    oldCenter,
    element.rotate,
  )
  const x = element.x + resizedCenter.x - w / 2
  const y = element.y + resizedCenter.y - h / 2

  return {
    h: roundCoordinate(h),
    w: roundCoordinate(w),
    x: roundCoordinate(x),
    y: roundCoordinate(y),
  }
}

function rotateVector(vector: { x: number; y: number }, degrees: number) {
  const radians = (degrees * Math.PI) / 180
  const cosine = Math.cos(radians)
  const sine = Math.sin(radians)
  return {
    x: vector.x * cosine - vector.y * sine,
    y: vector.x * sine + vector.y * cosine,
  }
}

function getArrowDelta(key: string, distance: number) {
  if (key === 'ArrowLeft') return { x: -distance, y: 0 }
  if (key === 'ArrowRight') return { x: distance, y: 0 }
  if (key === 'ArrowUp') return { x: 0, y: -distance }
  if (key === 'ArrowDown') return { x: 0, y: distance }
  return undefined
}

function getDirection(key: string) {
  return key.replace('Arrow', '').toLowerCase()
}

function getNudgeEdit(element: NormalizedElement, delta: { x: number; y: number }): ElementEdit {
  if (element.kind === 'line') {
    return {
      x1: roundCoordinate(element.x1 + delta.x),
      x2: roundCoordinate(element.x2 + delta.x),
      y1: roundCoordinate(element.y1 + delta.y),
      y2: roundCoordinate(element.y2 + delta.y),
    }
  }
  return {
    x: roundCoordinate(element.x + delta.x),
    y: roundCoordinate(element.y + delta.y),
  }
}

function makeSelectionBox(start: { x: number; y: number }, end: { x: number; y: number }) {
  return {
    h: Math.abs(end.y - start.y),
    w: Math.abs(end.x - start.x),
    x: Math.min(start.x, end.x),
    y: Math.min(start.y, end.y),
  }
}

function boxesIntersect(
  first: { h: number; w: number; x: number; y: number },
  second: { h: number; w: number; x: number; y: number },
) {
  return (
    first.x <= second.x + second.w &&
    first.x + first.w >= second.x &&
    first.y <= second.y + second.h &&
    first.y + first.h >= second.y
  )
}

function getFocusedElementKey(target: EventTarget) {
  return target instanceof Element
    ? target.closest('[data-element-key]')?.getAttribute('data-element-key') ?? undefined
    : undefined
}

function describeElementPosition(ref: SlideElementRef, edit?: ElementEdit) {
  if (!edit) {
    return `${getElementAccessibleLabel(ref.element)} moved.`
  }

  const x = ref.element.kind === 'line' ? edit.x1 : edit.x
  const y = ref.element.kind === 'line' ? edit.y1 : edit.y
  return x === undefined || y === undefined
    ? `${getElementAccessibleLabel(ref.element)} changed.`
    : `${getElementAccessibleLabel(ref.element)} moved to ${x}, ${y}.`
}
