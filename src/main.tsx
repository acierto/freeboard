import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./styles/fonts.css";
import "./styles/reset.css";
import "./styles/prototype.css";
import "./styles/supplemental.css";
import { App } from "./App";
import { setBackend } from "./lib/api";

async function start() {
  // The fixture backend exists only in the visual-verification build
  // (`vite build --mode visual`). In production this branch is removed.
  if (import.meta.env.MODE === "visual") {
    const params = new URLSearchParams(location.search);
    const theme = params.get("theme");
    if (theme === "light" || theme === "dark") document.documentElement.dataset.theme = theme;
    const { createFixtureBackend } = await import("./lib/fixtureBackend");
    setBackend(createFixtureBackend(params));
  }
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}

void start();
