import React from "react";
import ReactDOM from "react-dom/client";
import { AppRouter } from "@quarry/router";
import { RootRoutes } from "./app/RootRoutes";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <AppRouter>
      <RootRoutes />
    </AppRouter>
  </React.StrictMode>,
);
