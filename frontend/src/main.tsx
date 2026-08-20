import React from "react";
import ReactDOM from "react-dom/client";
import { AppRouter } from "@quarry/router";
import App from "./App";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <AppRouter>
      <App />
    </AppRouter>
  </React.StrictMode>,
);
