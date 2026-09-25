import {
  WEST_MONROE_CUSTOM_COLORS,
  WEST_MONROE_THEME_DISPLAY_NAME,
  WEST_MONROE_THEME_FAMILY,
  WEST_MONROE_THEME_FAMILY_ID,
  WEST_MONROE_THEME_NAME,
  WEST_MONROE_THEME_VERSION_ID,
} from './PowerpointConstants'
import { DEFAULT_THEME } from '../shared/PowerpointConstants'
import { escapeXml } from './PowerpointUtils'

export function applyDefaultThemeXml(xml: string) {
  const themed = xml
    .replace(/<a:theme([^>]*)name="[^"]*"/u, `<a:theme$1name="${WEST_MONROE_THEME_DISPLAY_NAME}"`)
    .replace(/<a:clrScheme\b[^>]*>[\s\S]*?<\/a:clrScheme>/u, buildThemeColorSchemeXml())
    .replace(/<a:fontScheme\b[^>]*>/u, '<a:fontScheme name="Arial">')
    .replace(/<a:majorFont><a:latin\b[^>]*\/>/u, '<a:majorFont><a:latin typeface="Arial"/>')
    .replace(/<a:minorFont><a:latin\b[^>]*\/>/u, '<a:minorFont><a:latin typeface="Arial"/>')
    .replace(
      /<thm15:themeFamily\b([^>]*)name="[^"]*"/u,
      `<thm15:themeFamily$1name="${WEST_MONROE_THEME_FAMILY}"`,
    )
    .replace(/(<thm15:themeFamily\b[^>]*\bid=")[^"]*"/u, `$1${WEST_MONROE_THEME_FAMILY_ID}"`)
    .replace(/(<thm15:themeFamily\b[^>]*\bvid=")[^"]*"/u, `$1${WEST_MONROE_THEME_VERSION_ID}"`)

  return upsertCustomColorList(themed)
}

function buildThemeColorSchemeXml() {
  const colorXml = Object.entries(DEFAULT_THEME)
    .map(([name, color]) => `<a:${name}><a:srgbClr val="${color}"/></a:${name}>`)
    .join('')
  return `<a:clrScheme name="${WEST_MONROE_THEME_NAME}">${colorXml}</a:clrScheme>`
}

function upsertCustomColorList(xml: string) {
  const customColorXml = `<a:custClrLst>${Object.entries(WEST_MONROE_CUSTOM_COLORS)
    .map(([name, color]) => `<a:custClr name="${escapeXml(name)}"><a:srgbClr val="${color}"/></a:custClr>`)
    .join('')}</a:custClrLst>`

  if (/<a:custClrLst>[\s\S]*?<\/a:custClrLst>/u.test(xml)) {
    return xml.replace(/<a:custClrLst>[\s\S]*?<\/a:custClrLst>/u, customColorXml)
  }

  if (xml.includes('<a:extLst>')) {
    return xml.replace('<a:extLst>', `${customColorXml}<a:extLst>`)
  }

  return xml.replace('</a:theme>', `${customColorXml}</a:theme>`)
}
