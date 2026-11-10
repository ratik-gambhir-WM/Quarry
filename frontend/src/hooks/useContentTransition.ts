import { useLayoutEffect, useRef } from "react";
import { dur, ease } from "@/components/ui/stealth-date/motion";

/** Animates a changed view in place without remounting its state or focus owner. */
export function useContentTransition(view: string) {
  const ref = useRef<HTMLDivElement>(null);
  const previousView = useRef(view);

  useLayoutEffect(() => {
    const changed = previousView.current !== view;
    previousView.current = view;
    const element = ref.current;
    if (
      !changed || !element || typeof element.animate !== "function" ||
      document.documentElement.dataset.motionInput === "keyboard"
    ) return;
    const preference = window.matchMedia("(prefers-reduced-motion: reduce)");
    if (preference.matches) return;

    const animation = element.animate(
      [{ opacity: 0, transform: "translateY(4px)" }, { opacity: 1, transform: "none" }],
      {
        id: "quarry-content-change",
        duration: dur.content * 1000,
        easing: `cubic-bezier(${ease.out.join(",")})`,
      },
    );
    // Cancellation is expected when a view changes again or the user switches input mode.
    void animation.finished.catch(() => undefined);
    const cancel = () => animation.cancel();
    const preferenceChanged = () => { if (preference.matches) cancel(); };
    document.addEventListener("keydown", cancel, { once: true, capture: true });
    preference.addEventListener("change", preferenceChanged);
    return () => {
      document.removeEventListener("keydown", cancel, true);
      preference.removeEventListener("change", preferenceChanged);
      cancel();
    };
  }, [view]);

  return ref;
}
