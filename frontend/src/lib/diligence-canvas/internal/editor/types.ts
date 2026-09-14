import type { HorizontalAlign } from '../shared/PowerpointTypes'

export type PlateTextLeaf = {
  bold?: boolean
  color?: string
  fontFamily?: string
  fontSize?: string
  italic?: boolean
  text: string
  underline?: boolean
}

export type PlateParagraph = {
  align?: HorizontalAlign
  children: PlateTextLeaf[]
  type: 'p'
}

export type PlateValue = PlateParagraph[]
