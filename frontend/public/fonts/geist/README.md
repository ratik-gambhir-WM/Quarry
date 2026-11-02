# Geist font assets

The app UI uses Geist Serif Regular 1.9.0, including text styled through the existing
`font-mono` compatibility token. This is a proportional serif by product choice.
The official Serif regular face is weight 400 with no italic face; browsers synthesize
existing bold and italic UI styles. The app preloads `GeistSerif-Regular.woff2`.

Source: https://registry.npmjs.org/geist/-/geist-1.9.0.tgz (official `geist` package).
The archive was verified against its npm SHA-512 integrity metadata before extraction.
The font binary is unmodified. It is served from the app origin for web and packaged
desktop use, so font rendering works offline.

Licensed under the SIL Open Font License 1.1; see LICENSE.txt. Retain the license when
updating fonts. Previously bundled Geist Sans and Geist Mono upright/italic variable
files are retained but are no longer registered or preloaded by the app UI.
