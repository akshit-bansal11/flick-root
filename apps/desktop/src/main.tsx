import { createRoot } from "react-dom/client";
import { APP_DISPLAY_NAME } from "./brand";
import { Launcher } from "./launcher/Launcher";
import "./tokens.css";
import "./launcher/launcher.css";

document.title = APP_DISPLAY_NAME;

const container = document.getElementById("root");
if (container) {
  createRoot(container).render(<Launcher />);
}
