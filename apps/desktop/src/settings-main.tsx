import { createRoot } from "react-dom/client";
import { APP_DISPLAY_NAME } from "./brand";
import { SettingsRoot } from "./settings/SettingsApp";
import "./tokens.css";

document.title = APP_DISPLAY_NAME;
// Rust adds ?mica=1 when the window has a Mica backdrop; without it Settings paints opaque.
if (new URLSearchParams(location.search).has("mica")) {
  document.documentElement.dataset.backdrop = "mica";
}

const container = document.getElementById("root");
if (container) {
  createRoot(container).render(<SettingsRoot />);
}
