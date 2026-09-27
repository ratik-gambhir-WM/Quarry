import {
  DEGREES_PER_CIRCLE,
  EMU_PER_DEGREE,
  EMU_PER_INCH,
  DEGREES_PER_HALF_CIRCLE,
  GEOMETRY_DECIMAL_PLACES,
  ZERO_ANGLE_TOLERANCE,
} from '../shared/PowerpointConstants'
import type { XmlNode } from '../shared/PowerpointTypes'
import type { TransformMatrix } from './PowerpointImportTypes'
import { emuToPx, firstDefined, round } from './PowerpointImportUtils'
import { child, findTransformNode } from './PowerpointXml'

export function extractElementTransform(
  node: XmlNode,
  matrix: TransformMatrix,
  placeholderSources: XmlNode[] = [],
) {
  const xfrm =
    node.tag === 'p:graphicFrame'
      ? child(node, 'p:xfrm')
      : findTransformNode(child(node, 'p:spPr')) ?? firstDefined(
          placeholderSources.map((source) => findTransformNode(child(source, 'p:spPr'))),
        )
  const off = child(xfrm, 'a:off')?.attributes ?? {}
  const ext = child(xfrm, 'a:ext')?.attributes ?? {}
  const rawX = Number(off.x) || 0
  const rawY = Number(off.y) || 0
  const rawW = Number(ext.cx) || 0
  const rawH = Number(ext.cy) || 0
  const transformedCenter = transformPoint(matrix, rawX + rawW / 2, rawY + rawH / 2)
  const matrixScaleX = Math.hypot(matrix.a, matrix.b)
  const matrixScaleY = Math.hypot(matrix.c, matrix.d)
  const w = Math.abs(matrixScaleX * rawW)
  const h = Math.abs(matrixScaleY * rawH)
  const x = transformedCenter.x - w / 2
  const y = transformedCenter.y - h / 2
  const parentRotation = (Math.atan2(matrix.b, matrix.a) * DEGREES_PER_HALF_CIRCLE) / Math.PI
  const ownRotation = xfrm?.attributes?.rot ? Number(xfrm.attributes.rot) / EMU_PER_DEGREE : 0
  const rotation = normalizeDegrees(parentRotation + ownRotation)
  const matrixIsReflected = matrix.a * matrix.d - matrix.b * matrix.c < 0

  return {
    xPx: emuToPx(x),
    yPx: emuToPx(y),
    widthPx: emuToPx(w),
    heightPx: emuToPx(h),
    rotation,
    flipH: xfrm?.attributes?.flipH === '1',
    flipV: (xfrm?.attributes?.flipV === '1') !== matrixIsReflected,
    xInches: round(x / EMU_PER_INCH, GEOMETRY_DECIMAL_PLACES),
    yInches: round(y / EMU_PER_INCH, GEOMETRY_DECIMAL_PLACES),
    widthInches: round(w / EMU_PER_INCH, GEOMETRY_DECIMAL_PLACES),
    heightInches: round(h / EMU_PER_INCH, GEOMETRY_DECIMAL_PLACES),
  }
}

export function groupTransform(node: XmlNode): TransformMatrix {
  const xfrm = findTransformNode(child(node, 'p:grpSpPr'))
  const off = child(xfrm, 'a:off')?.attributes ?? {}
  const ext = child(xfrm, 'a:ext')?.attributes ?? {}
  const chOff = child(xfrm, 'a:chOff')?.attributes ?? {}
  const chExt = child(xfrm, 'a:chExt')?.attributes ?? {}
  const extCx = Number(ext.cx) || 0
  const extCy = Number(ext.cy) || 0
  const chExtCx = Number(chExt.cx) || extCx || 1
  const chExtCy = Number(chExt.cy) || extCy || 1
  const scaleX = extCx ? extCx / chExtCx : 1
  const scaleY = extCy ? extCy / chExtCy : 1
  const offX = Number(off.x) || 0
  const offY = Number(off.y) || 0
  const base = {
    a: scaleX,
    b: 0,
    c: 0,
    d: scaleY,
    e: offX - (Number(chOff.x) || 0) * scaleX,
    f: offY - (Number(chOff.y) || 0) * scaleY,
  }
  const centerX = offX + extCx / 2
  const centerY = offY + extCy / 2
  const flip = scaleAround(
    centerX,
    centerY,
    xfrm?.attributes?.flipH === '1' ? -1 : 1,
    xfrm?.attributes?.flipV === '1' ? -1 : 1,
  )
  const rotation = rotateAround(
    centerX,
    centerY,
    xfrm?.attributes?.rot ? Number(xfrm.attributes.rot) / EMU_PER_DEGREE : 0,
  )

  return multiplyTransform(rotation, multiplyTransform(flip, base))
}

export function identityTransform(): TransformMatrix {
  return {
    a: 1,
    b: 0,
    c: 0,
    d: 1,
    e: 0,
    f: 0,
  }
}

export function composeTransform(parent: TransformMatrix, childTransform: TransformMatrix): TransformMatrix {
  return multiplyTransform(parent, childTransform)
}

function multiplyTransform(left: TransformMatrix, right: TransformMatrix): TransformMatrix {
  return {
    a: left.a * right.a + left.c * right.b,
    b: left.b * right.a + left.d * right.b,
    c: left.a * right.c + left.c * right.d,
    d: left.b * right.c + left.d * right.d,
    e: left.a * right.e + left.c * right.f + left.e,
    f: left.b * right.e + left.d * right.f + left.f,
  }
}

function transformPoint(matrix: TransformMatrix, x: number, y: number) {
  return {
    x: matrix.a * x + matrix.c * y + matrix.e,
    y: matrix.b * x + matrix.d * y + matrix.f,
  }
}

function rotateAround(centerX: number, centerY: number, degrees: number): TransformMatrix {
  if (!degrees) {
    return identityTransform()
  }

  const radians = (degrees * Math.PI) / DEGREES_PER_HALF_CIRCLE
  const cosine = Math.cos(radians)
  const sine = Math.sin(radians)
  return {
    a: cosine,
    b: sine,
    c: -sine,
    d: cosine,
    e: centerX - cosine * centerX + sine * centerY,
    f: centerY - sine * centerX - cosine * centerY,
  }
}

function scaleAround(
  centerX: number,
  centerY: number,
  scaleX: number,
  scaleY: number,
): TransformMatrix {
  return {
    a: scaleX,
    b: 0,
    c: 0,
    d: scaleY,
    e: centerX * (1 - scaleX),
    f: centerY * (1 - scaleY),
  }
}

function normalizeDegrees(value: number) {
  const normalized = ((value % DEGREES_PER_CIRCLE) + DEGREES_PER_CIRCLE) % DEGREES_PER_CIRCLE
  return Math.abs(normalized) < ZERO_ANGLE_TOLERANCE
    ? 0
    : round(normalized, GEOMETRY_DECIMAL_PLACES)
}
