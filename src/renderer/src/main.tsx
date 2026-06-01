import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { StudioProvider } from "./state/StudioContext";
import "./index.css";

const rootElement = document.getElementById("root");
if (rootElement === null) {
  throw new Error("#root element not found");
}

createRoot(rootElement).render(
  <StrictMode>
    <StudioProvider>
      <App />
    </StudioProvider>
  </StrictMode>,
);
