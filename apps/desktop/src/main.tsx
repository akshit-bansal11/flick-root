import { createRoot } from "react-dom/client";
import { APP_DISPLAY_NAME } from "./brand";

document.title = APP_DISPLAY_NAME;

const container = document.getElementById("root");
if (container) {
  createRoot(container).render(<div />);
}
