import type { XmlNode } from '../shared/PowerpointTypes'
import type { ExtractedTextBodyRecord } from './PowerpointImportTypes'

export function parseXml(xml: string): XmlNode {
  const root: XmlNode = { tag: '#document', children: [] }
  const stack = [root]
  const tokens = xml.match(/<!\[CDATA\[[\s\S]*?\]\]>|<!--[\s\S]*?-->|<\?[\s\S]*?\?>|<\/?[^>]+>|[^<]+/g) ?? []

  for (const token of tokens) {
    if (token.startsWith('<?') || token.startsWith('<!--') || token.startsWith('<!DOCTYPE')) {
      continue
    }

    if (token.startsWith('</')) {
      stack.pop()
      continue
    }

    if (token.startsWith('<![CDATA[')) {
      appendText(stack.at(-1), token.slice(9, -3))
      continue
    }

    if (token.startsWith('<')) {
      const selfClosing = /\/>\s*$/u.test(token)
      const body = token.slice(1, selfClosing ? -2 : -1).trim()
      const spaceIndex = body.search(/\s/u)
      const tag = spaceIndex === -1 ? body : body.slice(0, spaceIndex)
      const attributes = parseAttributes(spaceIndex === -1 ? '' : body.slice(spaceIndex + 1))
      const node: XmlNode = { tag, attributes, children: [] }
      stack.at(-1)?.children?.push(node)

      if (!selfClosing) {
        stack.push(node)
      }
      continue
    }

    appendText(stack.at(-1), decodeXml(token))
  }

  return root
}

function parseAttributes(input: string) {
  const attributes: Record<string, string> = {}
  const pattern = /([\w:.-]+)\s*=\s*(?:"([^"]*)"|'([^']*)')/gu
  let match: RegExpExecArray | null

  while ((match = pattern.exec(input))) {
    attributes[match[1]] = decodeXml(match[2] ?? match[3] ?? '')
  }

  return attributes
}

function appendText(node: XmlNode | undefined, text: string) {
  if (!node) {
    return
  }

  const decoded = decodeXml(text)
  if (!decoded) {
    return
  }

  node.text = `${node.text ?? ''}${decoded}`
}

export function collectRelationshipIds(node: XmlNode): string[] {
  return [
    ...Object.entries(node.attributes ?? {})
      .filter(([name]) => name === 'r:id' || name === 'r:embed' || name === 'r:link')
      .map(([, value]) => value),
    ...(node.children ?? []).flatMap(collectRelationshipIds),
  ]
}

export function paragraphText(textBody: ExtractedTextBodyRecord | undefined) {
  const lines = (textBody?.paragraphs ?? [])
    .map((paragraph) => (paragraph.runs ?? []).map((run) => run.text ?? '').join(''))
    .filter((line) => line.trim())
  return lines.length ? lines.join('\n') : textBody?.plainText?.trim() ?? ''
}

export function child(node: XmlNode | undefined, tag: string) {
  return node?.children?.find((candidate) => candidate.tag === tag)
}

export function children(node: XmlNode | undefined, tag: string) {
  return node?.children?.filter((candidate) => candidate.tag === tag) ?? []
}

export function hasChild(node: XmlNode | undefined, tag: string) {
  return !!child(node, tag)
}

export function descendants(node: XmlNode | undefined, tag: string): XmlNode[] {
  if (!node) {
    return []
  }

  return [
    ...(node.tag === tag ? [node] : []),
    ...(node.children ?? []).flatMap((candidate) => descendants(candidate, tag)),
  ]
}

export function findDescendant(node: XmlNode | undefined, tag: string) {
  return descendants(node, tag)[0]
}

export function findTransformNode(node: XmlNode | undefined) {
  return child(node, 'a:xfrm')
}

function decodeXml(input: string) {
  return input
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&apos;/g, "'")
    .replace(/&amp;/g, '&')
}
