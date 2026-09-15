import { POINTS_TO_SLIDE_UNITS } from '../shared/PowerpointConstants'

export function getTextEditorContentScale(fontScale: number) {
  return Math.max(fontScale * POINTS_TO_SLIDE_UNITS, 0.01)
}
