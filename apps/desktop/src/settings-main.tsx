import { createRoot } from "react-dom/client";
import { APP_DISPLAY_NAME } from "./brand";
import { FirstRun } from "./firstrun/FirstRun";
import { SettingsApp } from "./settings/SettingsApp";
import "./tokens.css";

document.title = APP_DISPLAY_NAME;

const container = document.getElementById("root");
if (container) {
  createRoot(container).render(location.hash === "#first-run" ? <FirstRun /> : <SettingsApp />);
}
