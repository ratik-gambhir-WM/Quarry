export const DEFAULT_WIDTH_PX = 1280
export const DEFAULT_HEIGHT_PX = 720
export const DEFAULT_FONT_FACE = 'Arial'
export const EMU_PER_INCH = 914400
export const PX_PER_INCH = 96
export const POINTS_TO_SLIDE_UNITS = PX_PER_INCH / 72
export const SLIDE_TEXT_LINE_HEIGHT = 1.05
export const DEFAULT_THEME: Record<string, string> = {
  dk1: '070154',
  lt1: 'FFFFFF',
  dk2: '0047FF',
  lt2: 'F6EB20',
  accent1: 'F900D3',
  accent2: '50658E',
  accent3: 'CED7E6',
  accent4: 'E8EEF8',
  accent5: '00E8FA',
  accent6: '00A3FF',
  hlink: '0563C1',
  folHlink: '954F72',
}
export const shapeAliases: Record<string, string> = {
  rect: 'rect',
  roundedRect: 'roundRect',
  roundRect: 'roundRect',
  ellipse: 'ellipse',
  oval: 'ellipse',
  diamond: 'diamond',
  chevron: 'chevron',
  database: 'flowChartMagneticDisk',
  cylinder: 'flowChartMagneticDisk',
  flowChartMagneticDisk: 'flowChartMagneticDisk',
  line: 'line',
}
