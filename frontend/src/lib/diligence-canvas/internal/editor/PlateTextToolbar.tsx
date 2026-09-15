import { useEditorRef, useEditorSelector } from 'platejs/react'
import { setAlign } from '@platejs/basic-styles'

import type { HorizontalAlign } from '../shared/PowerpointTypes'
import type { ResolvedTextEditorOptions } from './textEditorConfig'

const TOOLBAR_CONTROL_CLASSES = [
  'min-h-[1.9rem] rounded-[0.35rem] border border-white/20 bg-[rgba(255,255,255,0.08)]',
  'text-[0.72rem] text-inherit',
  'focus-visible:border-[#f3c316] focus-visible:outline-2 focus-visible:outline-offset-1',
  'focus-visible:outline-[#f3c316]/30',
].join(' ')

const TOOLBAR_BUTTON_CLASSES = [
  TOOLBAR_CONTROL_CLASSES,
  'min-w-[1.9rem] cursor-pointer px-[0.4rem] py-1',
  'hover:border-[#f3c316]',
  'aria-pressed:border-[#f3c316] aria-pressed:bg-[#f3c316]/20 aria-pressed:text-[#f3c316]',
].join(' ')

export function PlateTextToolbar({
  options,
}: {
  options: ResolvedTextEditorOptions
}) {
  const editor = useEditorRef()
  const marks = useEditorSelector((currentEditor) => currentEditor.api.marks() ?? {}, [])
  const selectedColor = colorValue(marks.color)
  const selectedFontFamily = stringValue(marks.fontFamily)
  const selectedFontSize = fontSizeValue(marks.fontSize)

  const applyMark = (key: 'color' | 'fontFamily' | 'fontSize', value: string) => {
    editor.tf.addMarks({ [key]: value })
    editor.tf.focus()
  }
  const setAlignment = (align: HorizontalAlign) => {
    setAlign(editor, align, { at: [] })
    editor.tf.focus()
  }

  return (
    <div
      aria-label="Text formatting"
      className="absolute right-0 bottom-[calc(100%+0.45rem)] z-[2] flex min-w-max items-center gap-1 rounded-[0.55rem] border border-white/20 bg-slate-900/95 p-[0.35rem] font-sans text-white shadow-[0_0.45rem_1.25rem_rgba(15,23,42,0.28)] motion-reduce:[&_*]:transition-none"
      role="toolbar"
    >
      {(['bold', 'italic', 'underline'] as const).map((mark) => (
        <button
          aria-label={capitalize(mark)}
          aria-pressed={marks[mark] === true}
          className={TOOLBAR_BUTTON_CLASSES}
          key={mark}
          onMouseDown={(event) => {
            event.preventDefault()
            editor.tf.toggleMark(mark)
          }}
          type="button"
        >
          {mark === 'bold' ? <strong>B</strong> : mark === 'italic' ? <em>I</em> : <u>U</u>}
        </button>
      ))}

      <label className="flex items-center gap-1">
        <span className="sr-only">Font</span>
        <select
          aria-label="Font family"
          className={`${TOOLBAR_CONTROL_CLASSES} max-w-[7.5rem] cursor-pointer px-[0.35rem] py-[0.2rem]`}
          onChange={(event) => applyMark('fontFamily', event.currentTarget.value)}
          value={selectedFontFamily}
        >
          <option value="">Font</option>
          {selectedFontFamily && !options.fontFamilies.includes(selectedFontFamily) ? (
            <option value={selectedFontFamily}>{selectedFontFamily}</option>
          ) : null}
          {options.fontFamilies.map((fontFamily) => (
            <option key={fontFamily} value={fontFamily}>{fontFamily}</option>
          ))}
        </select>
      </label>

      <label className="flex items-center gap-1">
        <span className="sr-only">Size</span>
        <select
          aria-label="Font size"
          className={`${TOOLBAR_CONTROL_CLASSES} max-w-[7.5rem] cursor-pointer px-[0.35rem] py-[0.2rem]`}
          onChange={(event) => applyMark('fontSize', `${event.currentTarget.value}px`)}
          value={selectedFontSize}
        >
          <option value="">Size</option>
          {selectedFontSize && !options.fontSizes.some((size) => String(size) === selectedFontSize) ? (
            <option value={selectedFontSize}>{selectedFontSize}</option>
          ) : null}
          {options.fontSizes.map((fontSize) => (
            <option key={fontSize} value={fontSize}>{fontSize}</option>
          ))}
        </select>
      </label>

      <label className="flex items-center gap-1">
        <span className="sr-only">Color</span>
        <select
          aria-label="Font color"
          className={`${TOOLBAR_CONTROL_CLASSES} max-w-[7.5rem] cursor-pointer px-[0.35rem] py-[0.2rem]`}
          onChange={(event) => applyMark('color', `#${event.currentTarget.value}`)}
          value={selectedColor}
        >
          <option value="">Color</option>
          {selectedColor && !options.colors.includes(selectedColor) ? (
            <option value={selectedColor}>#{selectedColor}</option>
          ) : null}
          {options.colors.map((color) => (
            <option key={color} value={color}>#{color}</option>
          ))}
        </select>
      </label>

      <div aria-label="Text alignment" className="flex items-center gap-1" role="group">
        {(['left', 'center', 'right'] as const).map((align) => (
          <button
            aria-label={`Align ${align}`}
            className={TOOLBAR_BUTTON_CLASSES}
            key={align}
            onMouseDown={(event) => {
              event.preventDefault()
              setAlignment(align)
            }}
            type="button"
          >
            {align === 'left' ? 'L' : align === 'center' ? 'C' : 'R'}
          </button>
        ))}
      </div>
    </div>
  )
}

function capitalize(value: string) {
  return `${value[0].toUpperCase()}${value.slice(1)}`
}

function fontSizeValue(value: unknown) {
  if (typeof value !== 'string' && typeof value !== 'number') return ''
  const numeric = Number.parseFloat(String(value))
  return Number.isFinite(numeric) ? String(numeric) : ''
}

function colorValue(value: unknown) {
  if (typeof value !== 'string') return ''
  const normalized = value.replace(/^#/u, '').toUpperCase()
  return /^[0-9A-F]{6}$/u.test(normalized) ? normalized : ''
}

function stringValue(value: unknown) {
  return typeof value === 'string' ? value : ''
}
