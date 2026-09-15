import { describe, expect, it } from 'vitest'

import type { DiligenceCanvasDocument } from '@/contracts/diligenceCanvas'
import { applyElementEdit, applyElementEditToInput } from '@/lib/diligence-canvas/internal/canvas/edits'
import { buildSlideCanvasModel } from '@/lib/diligence-canvas/internal/canvas/model'

const document: DiligenceCanvasDocument = {
  presentation: {
    preserveElementOrder: true,
    showBranding: false,
    slides: [{
      backgroundColor: 'FFFFFF',
      elements: [{
        align: 'left',
        fill: 'FFFFFF',
        fontFace: 'Aptos',
        fontSize: 18,
        h: 100,
        id: 'text-1',
        runs: [{
          color: '111827',
          fontFace: 'Aptos',
          fontSize: 18,
          text: 'Original',
        }],
        stroke: 'FFFFFF',
        strokeWidth: 0,
        text: 'Original',
        type: 'text',
        w: 300,
        x: 10,
        y: 20,
      }],
      height: 720,
      id: 'slide-1',
      name: 'Slide 1',
      width: 1280,
    }],
    title: 'Test presentation',
  },
}

describe('text run edits', () => {
  it('keeps the plain-text replacement path compatible with line breaks', () => {
    const ref = buildSlideCanvasModel(document).elementRefs[0]
    const edited = applyElementEdit(ref.element, { text: 'First\nSecond' })

    expect(edited.kind).toBe('text')
    if (edited.kind !== 'text') return
    expect(edited.text).toBe('First\nSecond')
    expect(edited.runs.map(({ breakLine, text }) => ({ breakLine, text }))).toEqual([
      { breakLine: true, text: 'First' },
      { breakLine: false, text: 'Second' },
    ])
  })

  it('writes partial formatting and alignment into an immutable JSON replacement', () => {
    const ref = buildSlideCanvasModel(document).elementRefs[0]
    const runs = [
      {
        bold: false,
        color: '111827',
        fontFace: 'Aptos',
        fontSize: 18,
        italic: false,
        text: 'Part ',
        underline: false,
      },
      {
        bold: true,
        color: 'DC2626',
        fontFace: 'Georgia',
        fontSize: 24,
        italic: true,
        text: 'styled',
        underline: true,
      },
    ]

    const next = applyElementEditToInput(document, ref, {
      align: 'right',
      runs,
      text: 'Part styled',
    })
    const raw = next.presentation.slides[0].elements[0]

    expect(next).not.toBe(document)
    expect(document.presentation.slides[0].elements[0].text).toBe('Original')
    expect(raw.text).toBe('Part styled')
    expect(raw.align).toBe('right')
    expect(raw.runs).toEqual(runs)

    const rebuilt = buildSlideCanvasModel(next).elementRefs[0].element
    expect(rebuilt.kind).toBe('text')
    if (rebuilt.kind !== 'text') return
    expect(rebuilt.runs).toEqual(runs)
    expect(rebuilt.align).toBe('right')
  })
})
