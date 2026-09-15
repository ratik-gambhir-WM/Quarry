import {
  forwardRef,
  useImperativeHandle,
  useMemo,
  useRef,
  type CSSProperties,
  type Ref,
} from 'react'
import { deserializeHtml } from 'platejs'
import { Plate, PlateContent, usePlateEditor } from 'platejs/react'

import type {
  NormalizedShapeElement,
  NormalizedTextElement,
} from '../shared/PowerpointTypes'
import { SLIDE_TEXT_LINE_HEIGHT } from '../shared/PowerpointConstants'
import { fromPlateValue, sanitizePlateValue, toPlateValue, type RichTextEdit } from './plateValue'
import { plateTextPlugins } from './platePlugins'
import { PlateTextToolbar } from './PlateTextToolbar'
import {
  resolveTextEditorOptions,
  type TextEditorOptions,
} from './textEditorConfig'
import type { PlateValue } from './types'

type EditableTextElement = NormalizedShapeElement | NormalizedTextElement

export type PlateTextEditorHandle = {
  getDraft: () => RichTextEdit
}

type PlateTextEditorProps = {
  caretColor?: string
  contentRef?: Ref<HTMLDivElement>
  contentStyle?: CSSProperties
  element: EditableTextElement
  hideText?: boolean
  onDraftChange?: () => void
  options?: TextEditorOptions
}

export const PlateTextEditor = forwardRef<PlateTextEditorHandle, PlateTextEditorProps>(
function PlateTextEditor({
  caretColor,
  contentRef,
  contentStyle,
  element,
  hideText = false,
  onDraftChange,
  options,
}, ref) {
  const initialValue = useMemo(() => toPlateValue(element), [element])
  const draftRef = useRef<PlateValue>(initialValue)
  const resolvedOptions = useMemo(() => resolveTextEditorOptions(options), [options])
  const editor = usePlateEditor({
    plugins: plateTextPlugins,
    value: initialValue,
  })
  useImperativeHandle(ref, () => ({
    getDraft: () => fromPlateValue(draftRef.current, element),
  }), [element])

  return (
    <Plate
      editor={editor}
      onValueChange={({ value }) => {
        draftRef.current = value as PlateValue
        onDraftChange?.()
      }}
    >
      <PlateTextToolbar options={resolvedOptions} />
      <div
        className="absolute top-0 left-0 box-border min-w-0 origin-top-left overflow-hidden"
        ref={contentRef}
        style={contentStyle}
      >
        <PlateContent
          aria-label="Edit slide text"
          autoFocus
          className="min-h-full w-full cursor-text bg-transparent caret-current outline-none [overflow-wrap:anywhere] selection:bg-blue-600/20"
          data-text-visibility={hideText ? 'snapshot' : 'draft'}
          onPaste={(event) => {
            const html = event.clipboardData.getData('text/html')
            if (!html) return
            event.preventDefault()
            const document = new DOMParser().parseFromString(html, 'text/html')
            document.querySelectorAll('script, style, img, video, audio, iframe, object, embed')
              .forEach((node) => node.remove())
            const fragment = deserializeHtml(editor, { element: document.body })
            editor.tf.insertFragment(sanitizePlateValue(fragment, element))
          }}
          placeholder="Type text"
          role="textbox"
          spellCheck={false}
          style={{
            caretColor,
            lineHeight: SLIDE_TEXT_LINE_HEIGHT,
            WebkitTextFillColor: hideText ? 'transparent' : undefined,
            whiteSpace: 'pre-wrap',
          }}
        />
      </div>
    </Plate>
  )
})
