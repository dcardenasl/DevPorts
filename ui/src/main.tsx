import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { bootLanguage } from "./boot";
import { App } from "./App";
import "./theme.css";
import "./styles.css";

const root = document.getElementById("root");
if (root === null) throw new Error("missing #root");
createRoot(root).render(
  <StrictMode>
    <App initialLanguage={bootLanguage} />
  </StrictMode>,
);
