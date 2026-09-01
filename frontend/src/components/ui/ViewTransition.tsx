import * as React from "react";

type ViewTransitionProps = React.ViewTransitionProps;

/**
 * Keeps the app usable while an already-running Vite session refreshes its
 * optimized React dependency after a canary upgrade.
 */
export function ViewTransition({ children, ...props }: ViewTransitionProps) {
  const NativeViewTransition = React.ViewTransition;

  if (NativeViewTransition === undefined) {
    return <>{children}</>;
  }

  return <NativeViewTransition {...props}>{children}</NativeViewTransition>;
}

export function markViewTransitionType(type: string) {
  React.addTransitionType?.(type);
}
