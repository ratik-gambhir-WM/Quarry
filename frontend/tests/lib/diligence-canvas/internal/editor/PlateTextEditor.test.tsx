// @vitest-environment happy-dom

import { createRef } from 'react'
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import type { NormalizedTextElement } from '@/lib/diligence-canvas/internal/shared/PowerpointTypes'
import { SLIDE_TEXT_LINE_HEIGHT } from '@/lib/diligence-canvas/internal/shared/PowerpointConstants'
import { PlateTextEditor, type PlateTextEditorHandle } from '@/lib/diligence-canvas/internal/editor/PlateTextEditor'

class TestResizeObserver implements ResizeObserver {
  disconnect() {}
  observe() {}
  unobserve() {}
}

globalThis.ResizeObserver = TestResizeObserver

describe('PlateTextEditor', () => {
  it('applies shape-level alignment through the toolbar draft', async () => {
    const ref = createRef<PlateTextEditorHandle>()
    render(<PlateTextEditor element={element} ref={ref} />)
    fireEvent.mouseDown(screen.getByRole('button', { name: 'Align center' }))

    await waitFor(() => {
      expect(ref.current?.getDraft()).toMatchObject({
        align: 'center',
        text: 'Alpha beta',
      })
    })
  })

  it('limits new color choices to the configured palette', () => {
    const { container } = render(
      <PlateTextEditor
        element={element}
        options={{ colors: ['123456', '#ABCDEF'] }}
      />,
    )

    const editor = within(container)
    expect(editor.getByRole('option', { name: '#123456' })).toBeTruthy()
    expect(editor.getByRole('option', { name: '#ABCDEF' })).toBeTruthy()
    expect(editor.queryByRole('option', { name: '#2563EB' })).toBeNull()
  })

  it('uses the same compact line spacing as the rendered SVG text', () => {
    const twoLineElement: NormalizedTextElement = {
      ...element,
      runs: [
        { ...element.runs[0], breakLine: true, text: 'Heading' },
        { ...element.runs[0], fontSize: 12, text: 'Body' },
      ],
      text: 'Heading\nBody',
    }
    const { container } = render(<PlateTextEditor element={twoLineElement} />)

    const editor = within(container).getByRole('textbox')
    const paragraphs = container.querySelectorAll<HTMLElement>('.slate-p')

    expect(editor.style.lineHeight).toBe(String(SLIDE_TEXT_LINE_HEIGHT))
    expect(editor.style.whiteSpace).toBe('pre-wrap')
    expect(paragraphs).toHaveLength(2)
    expect(paragraphs[0].style.fontSize).toBe('18px')
    expect(paragraphs[1].style.fontSize).toBe('12px')
    paragraphs.forEach((paragraph) => {
      expect(paragraph.style.lineHeight).toBe(String(SLIDE_TEXT_LINE_HEIGHT))
      expect(paragraph.style.margin).toBe('0px')
      expect(paragraph.style.padding).toBe('0px')
    })
  })
})

const element: NormalizedTextElement = {
  align: 'left',
  bold: false,
  borderRadius: 0,
  color: '111827',
  fill: 'transparent',
  fontFace: 'Aptos',
  fontSize: 18,
  h: 100,
  id: 'text-1',
  italic: false,
  kind: 'text',
  opacity: 1,
  padding: 4,
  rotate: 0,
  runs: [{
    bold: false,
    color: '111827',
    fontFace: 'Aptos',
    fontSize: 18,
    italic: false,
    text: 'Alpha beta',
    underline: false,
  }],
  sourcePath: 'presentation.slides[0].elements[0]',
  stroke: 'transparent',
  strokeWidth: 0,
  text: 'Alpha beta',
  valign: 'top',
  w: 300,
  x: 10,
  y: 20,
}
