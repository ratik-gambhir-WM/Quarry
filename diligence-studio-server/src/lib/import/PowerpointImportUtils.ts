import { EMU_PER_INCH, PX_PER_INCH } from '../shared/PowerpointConstants'

export function firstDefined<T>(values: Array<T | undefined>) {
  return values.find((value): value is T => value !== undefined)
}

export function positiveNumber(value: number | undefined, fallback: number) {
  return typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : fallback
}

export function positiveInt(value: string | undefined, fallback: number) {
  const parsed = Number(value)
  return Number.isFinite(parsed) && parsed > 0 ? Math.floor(parsed) : fallback
}

export function sum(values: number[]) {
  return values.reduce((total, value) => total + value, 0)
}

export function optionalNumber(value: number, fallback: number) {
  return value === fallback ? undefined : value
}

export function prune<T extends object>(input: T): T {
  return Object.fromEntries(
    Object.entries(input).filter(([, value]) => value !== undefined && value !== null && value !== ''),
  ) as T
}

export function round(value: number, digits = 2) {
  const factor = 10 ** digits
  return Math.round(value * factor) / factor
}

export function emuToPx(value: number) {
  return round((value / EMU_PER_INCH) * PX_PER_INCH)
}
