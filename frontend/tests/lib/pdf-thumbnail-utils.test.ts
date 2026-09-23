import { describe, expect, it } from "vitest"
import { resolvePdfAssetUrl } from "@/lib/pdf-thumbnail-utils"

describe("resolvePdfAssetUrl", () => {
  it("makes root-relative WASM paths absolute for blob workers", () => {
    expect(
      resolvePdfAssetUrl(
        "/node_modules/@embedpdf/pdfium/dist/pdfium.wasm",
        "http://localhost:1420/data-room"
      )
    ).toBe(
      "http://localhost:1420/node_modules/@embedpdf/pdfium/dist/pdfium.wasm"
    )
  })

  it("preserves an absolute production asset URL", () => {
    expect(
      resolvePdfAssetUrl(
        "https://assets.example.com/pdfium.wasm",
        "https://quarry.example.com/data-room"
      )
    ).toBe("https://assets.example.com/pdfium.wasm")
  })
})
