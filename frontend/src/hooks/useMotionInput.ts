import { useEffect } from "react";

/** Keeps keyboard-triggered interface changes instant, including portaled controls. */
export function useMotionInput() {
  useEffect(() => {
    const root = document.documentElement;
    const keyboard = () => { root.dataset.motionInput = "keyboard"; };
    const pointer = () => { root.dataset.motionInput = "pointer"; };
    document.addEventListener("keydown", keyboard, true);
    document.addEventListener("pointerdown", pointer, true);
    return () => {
      document.removeEventListener("keydown", keyboard, true);
      document.removeEventListener("pointerdown", pointer, true);
      delete root.dataset.motionInput;
    };
  }, []);
}
