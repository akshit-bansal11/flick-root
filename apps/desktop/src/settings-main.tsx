import { createRoot } from "react-dom/client";
import { APP_DISPLAY_NAME } from "./brand";
import { SettingsRoot } from "./settings/SettingsApp";
import "./tokens.css";

document.title = APP_DISPLAY_NAME;

const container = document.getElementById("root");
if (container) {
  createRoot(container).render(<SettingsRoot />);
}
