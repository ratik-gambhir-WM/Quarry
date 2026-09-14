export {
  applyElementEdit,
  applyElementEditsToInput,
  applyElementEditToInput,
  deleteElementsFromInput,
  type ElementEdit,
  type ElementEditRequest,
  type ElementMutationLocator,
} from './edits'
export {
  getElementAccessibleLabel,
  getElementGeometry,
  getRenderedLinePoints,
  roundCoordinate,
  type BoxGeometry,
  type ElementGeometry,
  type LineGeometry,
  type RenderedLinePoints,
} from './geometry'
export {
  buildSlideCanvasModel,
  type BuildSlideCanvasModelOptions,
  type SlideCanvasModel,
  type SlideElementRef,
} from './model'
export {
  buildNormalizedTextRuns,
  buildRawTextRuns,
  normalizeTextRuns,
  textFromNormalizedRuns,
} from './textRuns'
