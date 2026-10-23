import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// Vitest runs without globals, so Testing Library cannot register its own cleanup: unmount after every test here.
afterEach(cleanup);

// jsdom has no matchMedia; the app reads it for the system colour scheme and reduced motion.
// Tests that need a specific answer still stub it with vi.stubGlobal.
const mediaQuery = (media: string): MediaQueryList => ({
  matches: false,
  media,
  onchange: null,
  addListener: () => {},
  removeListener: () => {},
  addEventListener: () => {},
  removeEventListener: () => {},
  dispatchEvent: () => false,
});

if (typeof window.matchMedia !== "function") window.matchMedia = mediaQuery;
