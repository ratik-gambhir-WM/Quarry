export function toSvgColor(color: string) {
  if (!color || color === 'transparent') {
    return 'transparent'
  }

  return color.startsWith('#') ? color : `#${color}`
}

export function sanitizeSvgId(value: string) {
  return value.replace(/[^a-zA-Z0-9_-]/g, '-')
}

