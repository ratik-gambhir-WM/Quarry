export const DEFAULT_WIDTH_PX = 1280
export const DEFAULT_HEIGHT_PX = 720
export const DEFAULT_SLIDE_WIDTH_EMU = 12_192_000
export const DEFAULT_SLIDE_HEIGHT_EMU = 6_858_000
export const DEFAULT_FONT_FACE = 'Arial'
export const DEFAULT_BACKGROUND_COLOR = 'FFFFFF'
export const DEFAULT_TEXT_COLOR = '111827'
export const DEFAULT_STROKE_COLOR = '334155'
export const DEFAULT_SHAPE_FILL_COLOR = 'E5EEF8'
export const DEFAULT_TEXT_PADDING_PT = 8
export const DEFAULT_TEXT_FONT_SIZE_PT = 16
export const DEFAULT_SHAPE_FONT_SIZE_PT = 18
export const DEFAULT_TABLE_FONT_SIZE_PT = 12
export const DEFAULT_LINE_WIDTH_PT = 1
export const DEFAULT_CONNECTOR_WIDTH_PT = 1.5
export const DEFAULT_OPACITY = 1
export const MIN_ELEMENT_SIZE_PX = 1
export const MAX_ROUND_RECT_RATIO = 0.5
export const DEFAULT_ROUND_RECT_RATIO = 1 / 6
export const EMU_PER_INCH = 914400
export const EMU_PER_POINT = 12700
export const EMU_PER_DEGREE = 60_000
export const PX_PER_INCH = 96
export const POINTS_PER_INCH = 72
export const OOXML_PERCENT_SCALE = 100_000
export const OOXML_FONT_SIZE_SCALE = 100
export const DEGREES_PER_CIRCLE = 360
export const DEGREES_PER_HALF_CIRCLE = 180
export const GEOMETRY_DECIMAL_PLACES = 4
export const ZERO_ANGLE_TOLERANCE = 0.0001
export const SLIDE_BACKGROUND_Z_INDEX = -2_500
export const INHERITED_LAYOUT_Z_INDEX = -2_000
export const INHERITED_MASTER_Z_INDEX = -3_000
export const BACKGROUND_Z_INDEX_OFFSET = -1_000
export const BACKGROUND_ELEMENT_ID_BASE = 900_000
export const SLIDE_BOUNDS_TOLERANCE_PX = 1
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
