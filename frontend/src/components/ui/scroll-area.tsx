// Shared shadcn-style scroll area for standard surfaces, data grids and PDF viewports.
import { useMemo, type ComponentProps, type CSSProperties, type Ref } from "react";
import { ScrollArea as ScrollAreaPrimitive } from "@base-ui/react/scroll-area";
import { cn } from "@/lib/utils";

function ScrollAreaRoot({ className, ...props }: ComponentProps<typeof ScrollAreaPrimitive.Root>) {
  return <ScrollAreaPrimitive.Root data-slot="scroll-area" className={cn("relative", className)} {...props} />;
}

function ScrollAreaViewport({ className, ...props }: ComponentProps<typeof ScrollAreaPrimitive.Viewport>) {
  return <ScrollAreaPrimitive.Viewport data-slot="scroll-area-viewport" className={cn("rounded-[inherit] outline-none", className)} {...props} />;
}

function ScrollAreaContent(props: ComponentProps<typeof ScrollAreaPrimitive.Content>) {
  return <ScrollAreaPrimitive.Content data-slot="scroll-area-content" {...props} />;
}

function ScrollAreaThumb({ className, ...props }: ComponentProps<typeof ScrollAreaPrimitive.Thumb>) {
  return <ScrollAreaPrimitive.Thumb data-slot="scroll-area-thumb" className={cn("relative flex-1 rounded-full bg-foreground/20", className)} {...props} />;
}

function ScrollAreaScrollbar({ className, children, ...props }: ComponentProps<typeof ScrollAreaPrimitive.Scrollbar>) {
  return (
    <ScrollAreaPrimitive.Scrollbar
      data-slot="scroll-area-scrollbar"
      className={cn("flex touch-none select-none", className)}
      {...props}
    >
      {children ?? <ScrollAreaThumb />}
    </ScrollAreaPrimitive.Scrollbar>
  );
}

function ScrollAreaCorner(props: ComponentProps<typeof ScrollAreaPrimitive.Corner>) {
  return <ScrollAreaPrimitive.Corner data-slot="scroll-area-corner" {...props} />;
}

type ScrollAreaProps = Omit<ComponentProps<typeof ScrollAreaRoot>, "style"> & {
  style?: CSSProperties;
  orientation?: "vertical" | "horizontal" | "both";
  scrollFade?: boolean;
  scrollbarGutter?: boolean;
  scrollbarOverflowOnly?: boolean;
  viewportClassName?: string;
  viewportProps?: ComponentProps<typeof ScrollAreaViewport>;
  viewportRef?: Ref<HTMLDivElement>;
};

function composeRefs<T>(...refs: Array<Ref<T> | undefined>) {
  return (node: T | null) => {
    for (const ref of refs) {
      if (typeof ref === "function") ref(node);
      else if (ref) ref.current = node;
    }
  };
}

function ScrollArea({
  className, children, orientation = "both", scrollFade = false,
  scrollbarGutter = false, scrollbarOverflowOnly = false,
  viewportClassName, viewportProps, viewportRef, ...props
}: ScrollAreaProps) {
  const { className: viewportPropsClassName, ref: viewportPropsRef, ...resolvedViewportProps } = viewportProps ?? {};
  const composedViewportRef = useMemo(
    () => composeRefs(viewportPropsRef, viewportRef),
    [viewportPropsRef, viewportRef],
  );
  return (
    <ScrollAreaRoot
      className={cn("size-full min-h-0 [--fade-size:1.5rem]", className)}
      {...props}
    >
      <ScrollAreaViewport
        {...resolvedViewportProps}
        ref={composedViewportRef}
        className={cn(
          "size-full scrollbar-none focus-visible:ring-2 focus-visible:ring-ring",
          scrollFade && "mask-t-from-[calc(100%-min(var(--fade-size),var(--scroll-area-overflow-y-start)))] mask-r-from-[calc(100%-min(var(--fade-size),var(--scroll-area-overflow-x-end)))] mask-b-from-[calc(100%-min(var(--fade-size),var(--scroll-area-overflow-y-end)))] mask-l-from-[calc(100%-min(var(--fade-size),var(--scroll-area-overflow-x-start)))]",
          scrollbarGutter && orientation !== "vertical" && "pb-3.5",
          scrollbarGutter && orientation !== "horizontal" && "pe-3.5",
          viewportPropsClassName, viewportClassName,
        )}
      >
        <ScrollAreaContent>{children}</ScrollAreaContent>
      </ScrollAreaViewport>
      {(["vertical", "horizontal"] as const).filter((axis) => orientation === "both" || orientation === axis).map((axis) => (
        <ScrollAreaScrollbar
          key={axis}
          orientation={axis}
          keepMounted={!scrollbarOverflowOnly}
          className="m-1 bg-transparent opacity-0 transition-opacity delay-300 data-[orientation=vertical]:w-1.5 data-[orientation=horizontal]:h-1.5 data-[orientation=horizontal]:flex-col data-hovering:opacity-100 data-hovering:delay-0 data-hovering:duration-100 data-scrolling:opacity-100 data-scrolling:delay-0 data-scrolling:duration-100 data-[orientation=vertical]:not-data-has-overflow-y:hidden data-[orientation=horizontal]:not-data-has-overflow-x:hidden motion-reduce:transition-none"
        />
      ))}
      {orientation === "both" && <ScrollAreaCorner />}
    </ScrollAreaRoot>
  );
}

export { ScrollArea, ScrollAreaRoot, ScrollAreaViewport, ScrollAreaContent, ScrollAreaScrollbar, ScrollAreaThumb, ScrollAreaCorner };
export type { ScrollAreaProps };
