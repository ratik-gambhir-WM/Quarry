// @vitest-environment happy-dom

import { createRef } from 'react'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { NormalizedTextElement } from '@/lib/diligence-canvas/internal/shared/PowerpointTypes'
import { POINTS_TO_SLIDE_UNITS } from '@/lib/diligence-canvas/internal/shared/PowerpointConstants'
import { getTextEditorContentScale } from '@/lib/diligence-canvas/internal/svg/textEditorLayout'
import { SvgTextEditorOverlay } from '@/lib/diligence-canvas/internal/svg/SvgTextEditorOverlay'

class TestResizeObserver implements ResizeObserver {
  disconnect() {}
  observe() {}
  unobserve() {}
}

globalThis.ResizeObserver = TestResizeObserver
HTMLCanvasElement.prototype.getContext = vi.fn(() => null)
afterEach(() => {
  document.body.replaceChildren()
})

describe('SVG text editor overlay', () => {
  it('converts PowerPoint point sizes to the same units used by SVG text', () => {
    const fontScale = 0.75
    const contentScale = getTextEditorContentScale(fontScale)

    expect(contentScale).toBeCloseTo(POINTS_TO_SLIDE_UNITS * fontScale)
    expect(24 * contentScale).toBeCloseTo(24)
  })

  it('keeps the exact SVG text visible until the editor draft changes', async () => {
    const containerNode = document.createElement('div')
    const coordinateRoot = document.createElementNS('http://www.w3.org/2000/svg', 'g')
    containerNode.getBoundingClientRect = () => makeRect(0, 0, 800, 600)
    coordinateRoot.getScreenCTM = () => new DOMMatrix()
    const containerRef = createRef<HTMLDivElement>()
    const coordinateRootRef = createRef<SVGGElement>()
    Object.assign(containerRef, { current: containerNode })
    Object.assign(coordinateRootRef, { current: coordinateRoot })

    const view = render(
      <SvgTextEditorOverlay
        containerRef={containerRef}
        coordinateRootRef={coordinateRootRef}
        element={element}
        onCancel={vi.fn()}
        onCommit={vi.fn()}
        viewportTransform="translate(0 0) scale(1)"
      />,
    )

    const editor = await screen.findByRole('textbox', { name: 'Edit slide text' })
    const snapshot = view.container.querySelector('svg[aria-hidden="true"]')
    expect(snapshot?.textContent).toContain('Alpha beta')
    expect(editor.dataset.textVisibility).toBe('snapshot')
    expect(editor.style.caretColor).toBe('transparent')

    fireEvent.mouseDown(screen.getByRole('button', { name: 'Align center' }))

    await waitFor(() => {
      expect(view.container.querySelector('svg[aria-hidden="true"]')).toBeNull()
      expect(editor.dataset.textVisibility).toBe('draft')
      expect(editor.style.caretColor).toBe('#111827')
    })
  })
})

const element: NormalizedTextElement = {
  align: 'left',
  bold: false,
  borderRadius: 0,
  color: '111827',
  fill: 'transparent',
  fontFace: 'Arial',
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
    fontFace: 'Arial',
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

function makeRect(x: number, y: number, width: number, height: number): DOMRect {
  return {
    bottom: y + height,
    height,
    left: x,
    right: x + width,
    toJSON: () => ({}),
    top: y,
    width,
    x,
    y,
  }
}
