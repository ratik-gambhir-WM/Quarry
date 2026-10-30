/** Ark does not run its open-transition anchor callback when mounted already open. */
export function getFloatingPanelInitialPosition(
  size: { width: number; height: number },
  boundary?: HTMLElement | null,
  inset?: { right: number; bottom: number },
) {
  const rect = boundary?.getBoundingClientRect();
  const x = rect?.x ?? 0;
  const y = rect?.y ?? 0;
  const width = rect?.width ?? (typeof window === "undefined" ? size.width : window.innerWidth);
  const height = rect?.height ?? (typeof window === "undefined" ? size.height : window.innerHeight);
  return {
    x: Math.max(x, x + (inset ? width - size.width - inset.right : (width - size.width) / 2)),
    y: Math.max(y, y + (inset ? height - size.height - inset.bottom : (height - size.height) / 2)),
  };
}
