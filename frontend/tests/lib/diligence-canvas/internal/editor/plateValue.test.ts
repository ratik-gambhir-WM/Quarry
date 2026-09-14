import { describe, expect, it } from 'vitest'

import type { NormalizedTextElement, NormalizedTextRun } from '@/lib/diligence-canvas/internal/shared/PowerpointTypes'
import { fromPlateValue, toPlateValue } from '@/lib/diligence-canvas/internal/editor/plateValue'

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
  runs: [],
  sourcePath: 'presentation.slides[0].elements[0]',
  stroke: 'transparent',
  strokeWidth: 0,
  text: '',
  valign: 'top',
  w: 300,
  x: 10,
  y: 20,
}

describe('Plate value adapters', () => {
  it('round-trips mixed inline formatting without collapsing adjacent styles', () => {
    const source: NormalizedTextElement = {
      ...element,
      align: 'center',
      text: 'Alpha beta',
      runs: [
        run('Alpha ', { bold: false }),
        run('beta', { bold: true, color: 'DC2626', fontFace: 'Georgia', fontSize: 24 }),
      ],
    }

    const result = fromPlateValue(toPlateValue(source), source, { strict: true })

    expect(result).toEqual({ align: 'center', runs: source.runs, text: 'Alpha beta' })
  })

  it('preserves consecutive blank paragraphs and empty shapes', () => {
    const source: NormalizedTextElement = {
      ...element,
      text: 'First\n\nThird',
      runs: [
        run('First', { breakLine: true }),
        run('', { breakLine: true }),
        run('Third'),
      ],
    }

    expect(fromPlateValue(toPlateValue(source), source)).toEqual({
      align: 'left',
      runs: source.runs,
      text: 'First\n\nThird',
    })
    expect(fromPlateValue(toPlateValue(element), element).text).toBe('')

    const trailingBlank = fromPlateValue([
      { children: [{ text: 'First' }], type: 'p' },
      { children: [{ text: '' }], type: 'p' },
    ], element)
    expect(trailingBlank.text).toBe('First\n')
  })

  it('inherits missing supported marks and normalizes font and color values', () => {
    const result = fromPlateValue([
      {
        align: 'right',
        children: [
          { text: 'A' },
          { color: '#abc', fontSize: '20px', italic: true, text: 'B' },
        ],
        type: 'p',
      },
    ], element)

    expect(result).toEqual({
      align: 'right',
      runs: [
        run('A'),
        run('B', { color: 'AABBCC', fontSize: 20, italic: true }),
      ],
      text: 'AB',
    })
  })

  it('coalesces only adjacent leaves with identical styles', () => {
    const result = fromPlateValue([
      { children: [{ bold: true, text: 'A' }, { bold: true, text: 'B' }, { text: 'C' }], type: 'p' },
    ], element)

    expect(result.runs).toEqual([run('AB', { bold: true }), run('C')])
  })

  it('rejects unsupported nodes and marks in strict mode but strips them at runtime', () => {
    const unsupported = [
      { children: [{ href: 'https://example.com', text: 'safe text' }], type: 'heading' },
    ]

    expect(() => fromPlateValue(unsupported, element, { strict: true })).toThrow(
      'Unsupported Plate node',
    )
    expect(fromPlateValue(unsupported, element)).toMatchObject({
      runs: [run('safe text')],
      text: 'safe text',
    })
  })
})

function run(
  text: string,
  overrides: Partial<NormalizedTextRun> = {},
) {
  return runShape(text, overrides)
}

function runShape(text: string, overrides: Partial<NormalizedTextRun> = {}): NormalizedTextRun {
  return {
    bold: false,
    color: '111827',
    fontFace: 'Aptos',
    fontSize: 18,
    italic: false,
    text,
    underline: false,
    ...overrides,
  }
}
