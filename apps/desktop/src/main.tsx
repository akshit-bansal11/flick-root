import { createRoot } from "react-dom/client";
import { APP_DISPLAY_NAME } from "./brand";
import { Root } from "./Root";
import "./tokens.css";
import "./launcher/launcher.css";
import "./tray/tray.css";

document.title = APP_DISPLAY_NAME;

const container = document.getElementById("root");
if (container) {
  createRoot(container).render(<Root />);
}
