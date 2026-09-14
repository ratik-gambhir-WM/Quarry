import { PlateElement, type PlateElementProps } from 'platejs/react'

import { SLIDE_TEXT_LINE_HEIGHT } from '../shared/PowerpointConstants'

export function SlideTextParagraph(props: PlateElementProps) {
  const fontSize = getParagraphFontSize(props.element.children)

  return (
    <PlateElement
      {...props}
      style={{
        ...props.style,
        fontSize,
        lineHeight: SLIDE_TEXT_LINE_HEIGHT,
        margin: 0,
        padding: 0,
      }}
    />
  )
}

function getParagraphFontSize(children: unknown) {
  if (!Array.isArray(children)) return undefined

  const fontSizes = children.flatMap((child) => {
    if (typeof child !== 'object' || child === null || !('fontSize' in child)) {
      return []
    }
    const fontSize = Number.parseFloat(String(child.fontSize))
    return Number.isFinite(fontSize) ? [fontSize] : []
  })

  return fontSizes.length ? `${Math.max(...fontSizes)}px` : undefined
}
