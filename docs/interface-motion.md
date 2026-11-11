# Interface motion

Motion is part of building Quarry UI. Add it when it acknowledges an action, locates a popup,
or connects two visible states. A person should understand the change without waiting for it.
Quiet, consistent transitions are the default; frequently repeated work needs less motion.

## Existing implementation

- CSS tokens live in `frontend/src/index.css`. Motion's corresponding JavaScript tokens live in
  `frontend/src/components/ui/stealth-date/motion.ts`; keep shared timings and curves aligned.
- `quarry-popup-motion` tunes the existing menu, submenu, popover, tooltip, and select lifecycle.
  Entrances take 180 ms; exits take 120 ms. Origins still come from each primitive's trigger.
  Item-aligned Radix selects retain their deliberate no-animation behavior.
- `quarry-dialog-motion` and `quarry-backdrop-motion` use 260 ms entrances and 160 ms exits.
  Radix/Base UI retain responsibility for presence, inertness, focus trapping, and restoration.
- Shared tab panels reveal with a 180 ms opacity/4-pixel translation. Sidebar width changes use
  220 ms. Width is a bounded exception because collapsing the sidebar changes available workspace
  space; do not apply layout animation to every row or card.
- `useContentTransition(view)` animates an existing container only when its selected view changes.
  Deal Room sections and Assistant sidebar views use it. It preserves the container and its state,
  skips initial render, cancels interrupted animations, and falls back to immediate rendering when
  native animations are unavailable. Do not use it for keystrokes, streamed tokens, polling,
  pagination, or every document render.
- `useMotionInput()` is mounted once in the product root. Captured keydown/pointerdown events
  maintain `data-motion-input` on the document root, including interactions with portaled controls.
  Keyboard changes are instant for the opted-in surfaces, and keydown cancels active content motion.
- React View Transitions retain the feature-detecting `ViewTransition` wrapper. Existing Deals
  table/board sharing and lazy-page reveals use the CSS recipes; persistent chrome is excluded.

## Rules for new and changed UI

1. Name the purpose before animating. Use no motion for a change that would otherwise feel slower.
2. Reuse shared controls and tokens. Prefer CSS for simple changes and primitive-owned enter/exit
   states. Use the installed Motion library for gestures and interruptible layout work. The native
   content hook handles swaps without adding wrappers or remounting children. Do not add another
   animation library for a fade.
3. Prefer opacity and transform, with 4–8 pixels of travel or restrained scaling. Use strong ease-out
   on entrance (`--ease-out-expo`), finish exits sooner, and start feedback immediately. Avoid
   `transition: all`, arbitrary delays, large-area blur, bouncy product UI, and staggered data rows.
4. Keep keyboard actions instant. New CSS-driven interactive surfaces should join the modality
   opt-out in `index.css`; new JavaScript animations must check input mode and cancel on keyboard
   input. Preserve focus, semantics, hit targets, reading order, and immediate error feedback.
5. Respect `prefers-reduced-motion` at launch and when it changes. Clear delays and positional
   animation; content must remain fully visible and usable. Loading indicators must still convey
   progress through accessible status text when decorative movement is suppressed.
6. Preserve state and scroll. Do not introduce changing keys, duplicate state, delayed unmount
   timers, effects that set React state, or a second presence controller to make an animation play.
   A modal's existing library owns its lifecycle; a transition must not delay authorization changes.
7. Keep interruption safe. Repeated selection, quick open/close, Escape, and unmount must cancel or
   reverse promptly. Do not leave invisible content focusable or let stale animations reveal it.

## Verification and review

Run focused behavioral tests for changed lifecycle/state logic, both frontend type targets,
boundary checks, the full frontend suite, and applicable builds using the code-verification guide.
For visual-only changes, prefer browser inspection to tests that merely assert a CSS class.

Inspect pointer and keyboard paths, quick reversal, focus restoration, loading/error content,
supported themes and viewport sizes, reduced motion, and missing native-animation support.
Confirm that unrelated data updates and typing do not replay view motion. Check the browser console
and inspect actual animation durations when timing matters. Report any platform or preference path
that could not be exercised. Preserve existing tests for state ownership and API boundaries.

The animation-lifecycle choice follows [Radix's CSS animation guidance](https://www.radix-ui.com/primitives/docs/guides/animation).
The native content hook uses the browser's [Element.animate API](https://developer.mozilla.org/en-US/docs/Web/API/Element/animate).
