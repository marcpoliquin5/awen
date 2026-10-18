import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { startFringes } from "./fringes";
import "./style.css";

startFringes(document.getElementById("fringes") as HTMLCanvasElement);
createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
