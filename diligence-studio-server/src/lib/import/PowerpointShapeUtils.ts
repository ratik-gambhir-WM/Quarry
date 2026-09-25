import type { XmlNode } from '../shared/PowerpointTypes'
import { child, findDescendant } from './PowerpointXml'

export function extractNonVisual(node: XmlNode) {
  const nonVisualNode =
    child(node, 'p:nvSpPr') ??
    child(node, 'p:nvCxnSpPr') ??
    child(node, 'p:nvPicPr') ??
    child(node, 'p:nvGraphicFramePr')
  const cNvPr = child(nonVisualNode, 'p:cNvPr')
  const cNvSpPr = child(nonVisualNode, 'p:cNvSpPr')
  const placeholder = findDescendant(nonVisualNode, 'p:ph')
  return {
    id: Number(cNvPr?.attributes?.id) || undefined,
    name: cNvPr?.attributes?.name,
    description: cNvPr?.attributes?.descr || cNvPr?.attributes?.title,
    hidden: cNvPr?.attributes?.hidden === '1',
    isTextBox: cNvSpPr?.attributes?.txBox === '1',
    placeholder: placeholder
      ? {
          type: placeholder.attributes?.type,
          idx: placeholder.attributes?.idx,
        }
      : undefined,
  }
}
