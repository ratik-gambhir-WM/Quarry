import type { ReactNode } from "react";
import { BrowserRouter } from "react-router-dom";

export function AppRouter({ children }: { children: ReactNode }) {
  return <BrowserRouter useTransitions>{children}</BrowserRouter>;
}
