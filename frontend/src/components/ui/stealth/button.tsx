// Adapted from https://www.stealth.pm/ui/button; Quarry theme tokens and existing state owners.
"use client";
import { Button as BaseButton } from "@base-ui/react/button";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { cn } from "@/lib/utils";
import { ease } from "../stealth-date/motion";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
export type ButtonSize = "sm" | "md" | "lg";

const sizes: Record<
  ButtonSize,
  { box: string; gap: string; start: string; end: string }
> = {
  sm: {
    box: "h-7 rounded-md px-2 text-[12px] [&_svg:not([class*='size-'])]:size-3.5",
    gap: "gap-1.5",
    start: "pl-1.5",
    end: "pr-1.5",
  },
  md: {
    box: "h-8 rounded-lg px-2.5 text-[12.5px] [&_svg:not([class*='size-'])]:size-4",
    gap: "gap-1.5",
    start: "pl-2",
    end: "pr-2",
  },
  lg: {
    box: "h-9 rounded-lg px-3 text-[13px] [&_svg:not([class*='size-'])]:size-4",
    gap: "gap-2",
    start: "pl-2.5",
    end: "pr-2.5",
  },
};

// Inside a ButtonGroup, secondary and ghost buttons become flat segments: the
// group draws the border, the shadow, the hairlines and the hover wash.
const segment =
  "in-data-[slot=button-group]:rounded-none in-data-[slot=button-group]:border-0 in-data-[slot=button-group]:bg-transparent in-data-[slot=button-group]:shadow-none in-data-[slot=button-group]:hover:bg-transparent in-data-[slot=button-group]:data-popup-open:bg-transparent in-data-[slot=button-group]:active:scale-100 in-data-[slot=button-group]:focus-visible:-outline-offset-3 in-data-[slot=button-group]:focus-visible:rounded-[5px]";

const variants: Record<ButtonVariant, string> = {
  primary:
    "bg-action text-on-action shadow-sm hover:bg-action-hover data-popup-open:bg-action-hover",
  secondary:
    "border border-outline-variant bg-surface-container-lowest text-on-surface shadow-sm hover:border-outline hover:bg-surface-container-high data-popup-open:border-outline data-popup-open:bg-surface-container-high " +
    segment,
  ghost:
    "text-muted hover:bg-surface-container-high hover:text-on-surface data-popup-open:bg-surface-container-high data-popup-open:text-on-surface " +
    segment,
  danger:
    "border border-destructive/25 bg-destructive/10 text-destructive hover:border-destructive/45 hover:bg-destructive/15 data-popup-open:bg-destructive/15",
};

/**
 * The button's classes on their own, for putting the look on an element that
 * isn't a Button (a menu trigger, a label, a router link you render yourself).
 */
export function buttonVariants({
  variant = "secondary",
  size = "md",
  leading = false,
  trailing = false,
}: {
  variant?: ButtonVariant;
  size?: ButtonSize;
  leading?: boolean;
  trailing?: boolean;
} = {}) {
  const s = sizes[size];
  return cn(
    "group/button relative inline-flex shrink-0 select-none items-center justify-center whitespace-nowrap font-medium tracking-[-0.005em]",
    "touch-manipulation [-webkit-tap-highlight-color:transparent]",
    "outline-none focus-visible:outline-solid focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-primary",
    // Press: quick in (75ms), softer out (150ms). Busy buttons don't flinch.
    "transition-[background-color,border-color,color,scale] duration-150 ease-out motion-reduce:transition-none active:not-data-busy:scale-[0.97] active:not-data-busy:duration-75",
    "data-disabled:not-data-busy:pointer-events-none data-disabled:not-data-busy:opacity-50 data-busy:cursor-progress",
    // A 28px button still takes a 44px tap on touch screens.
    size === "sm" &&
      "pointer-coarse:after:absolute pointer-coarse:after:inset-x-0 pointer-coarse:after:-inset-y-2",
    s.box,
    leading && s.start,
    trailing && s.end,
    variants[variant],
  );
}

/** The busy glyph: a faint track and a quarter arc turning on it. Sized by its parent's svg rule. */
export function Spinner({ className, ...rest }: React.ComponentProps<"svg">) {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden
      focusable={false}
      className={cn(
        "shrink-0 animate-[spin_0.7s_linear_infinite] motion-reduce:animate-none",
        className,
      )}
      {...rest}
    >
      <circle
        cx="8"
        cy="8"
        r="5.75"
        stroke="currentColor"
        strokeOpacity="0.22"
        strokeWidth="1.5"
      />
      <path
        d="M8 2.25A5.75 5.75 0 0 1 13.75 8"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
      />
    </svg>
  );
}

type Common = {
  variant?: ButtonVariant;
  size?: ButtonSize;
  /** Icon before the label. Decorative; the label names the button. */
  leadingIcon?: React.ReactNode;
  /** Icon after the label: a chevron, an arrow, an external-link mark. */
  trailingIcon?: React.ReactNode;
};

export type ButtonProps = Omit<BaseButton.Props, "render" | "className"> &
  Common & {
    className?: string;
    loading?: boolean;
    loadingText?: string;
  };

export function Button({
  variant = "secondary",
  size = "md",
  leadingIcon,
  trailingIcon,
  loading = false,
  loadingText,
  disabled,
  className,
  children,
  ...rest
}: ButtonProps) {
  const busy = loading;
  return (
    <BaseButton
      data-variant={variant}
      data-size={size}
      // The caller owns request timing; busy feedback does not add another state lifecycle.
      data-busy={busy ? "" : undefined}
      aria-busy={busy || undefined}
      aria-label={
        typeof children === "string"
          ? busy && loadingText
            ? loadingText
            : children
          : undefined
      }
      disabled={disabled || busy}
      // Keep focus on the button while it works, so keyboard users aren't dropped to <body>.
      focusableWhenDisabled={busy && !disabled}
      className={cn(
        buttonVariants({
          variant,
          size,
          leading: !!leadingIcon,
          trailing: !!trailingIcon,
        }),
        className,
      )}
      {...rest}
    >
      <Content
        size={size}
        leadingIcon={leadingIcon}
        trailingIcon={trailingIcon}
        busy={busy}
        loadingText={loadingText}
      >
        {children}
      </Content>
    </BaseButton>
  );
}

function Content({
  size,
  leadingIcon,
  trailingIcon,
  busy,
  loadingText,
  children,
}: Common & {
  size: ButtonSize;
  busy: boolean;
  loadingText?: string;
  children?: React.ReactNode;
}) {
  const reduce = useReducedMotion();
  const gap = sizes[size].gap;
  const t = { duration: reduce ? 0.15 : 0.22, ease: ease.out };
  // The idle row leaves upward while the busy row arrives from below, so it reads
  // as one thing replacing another, never as the button blinking.
  const away = reduce
    ? { opacity: 0 }
    : { opacity: 0, y: -6, filter: "blur(2px)" };
  const below = reduce
    ? { opacity: 0 }
    : { opacity: 0, y: 6, filter: "blur(2px)" };
  const here = { opacity: 1, y: 0, filter: "blur(0px)" };

  return (
    // In a group the whole button can't shrink without tearing the border, so its content squashes instead.
    <span className="grid place-items-center transition-[scale] duration-100 ease-out in-data-[slot=button-group]:group-active/button:scale-[0.95]">
      {/* Holds room for the busy row, so the button is as wide as the longer of the two. */}
      <span
        aria-hidden
        className={cn(
          "invisible col-start-1 row-start-1 inline-flex items-center",
          gap,
        )}
      >
        <span className={size === "sm" ? "size-3.5" : "size-4"} />
        {loadingText}
      </span>
      <motion.span
        // With busy text showing, the idle label steps out of the accessible name too.
        aria-hidden={(busy && !!loadingText) || undefined}
        className={cn(
          "col-start-1 row-start-1 inline-flex min-w-0 items-center",
          gap,
        )}
        initial={false}
        animate={busy ? away : here}
        transition={t}
      >
        {leadingIcon && (
          <span aria-hidden className="inline-flex shrink-0">
            {leadingIcon}
          </span>
        )}
        <span>{children}</span>
        {trailingIcon && (
          <span aria-hidden className="inline-flex shrink-0">
            {trailingIcon}
          </span>
        )}
      </motion.span>
      <AnimatePresence initial={false}>
        {busy && (
          <motion.span
            key="busy"
            className={cn(
              "col-start-1 row-start-1 inline-flex items-center",
              gap,
            )}
            initial={below}
            animate={here}
            exit={{
              ...below,
              transition: { duration: reduce ? 0.1 : 0.16, ease: ease.out },
            }}
            transition={t}
          >
            <Spinner />
            {loadingText && <span>{loadingText}</span>}
          </motion.span>
        )}
      </AnimatePresence>
    </span>
  );
}
