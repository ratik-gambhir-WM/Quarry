# DiligenceCanvas (Quarry snapshot)

Copied from the standalone Diligence Canvas source on 2026-09-14. The source matched the
revalidated Quarry integration-plan baseline; Diligence Studio was at `c1665ad`, with `84fef3b`
as the minimum known-compatible app-scoped template retrieval revision.

The snapshot keeps the slide model, immutable edit helpers, SVG renderer, interactions, text
layout, shapes, Plate rich-text editor, normalization code, Tailwind utility styling, and bundled
brand asset. Presentation-provided geometry, colors, transforms, and fonts remain document data.

Quarry adaptations remove the standalone network API, import/export/download controls, API base
URL and fetch overrides, and public brand-logo override. Quarry owns transport through
`QuarryApi`; this library is a controlled, transport-free presentation editor. Source tests live
under `frontend/tests/lib/diligence-canvas/` rather than beside production code.

The host owns the document. Every drag, resize, text edit, or delete emits a complete immutable
replacement through `onChange`:

```tsx
import { DiligenceCanvas } from '@/lib/diligence-canvas/DiligenceCanvas'

<DiligenceCanvas value={document} onChange={setDocument} />
```

`resolveImageSource` remains an optional pure rendering hook. Quarry does not supply it for
hydrated template documents. The bundled West Monroe logo is fixed; `showBranding` can still
control whether the document's brand frame is rendered.

Rich-text editing supports bold, italic, underline, font color, font family, font size, and
whole-shape alignment. Pasted content is reduced to the subset preserved by the SVG and
PowerPoint-compatible model. The copied production entry intentionally exports only the
controlled editor and its transport-neutral types; lower-level implementation modules remain
internal to the snapshot.
