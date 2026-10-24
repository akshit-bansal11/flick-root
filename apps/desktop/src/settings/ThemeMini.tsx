import type { CSSProperties } from "react";
import type { ThemeRoles } from "../theme";

/** Inline style carrying CSS custom properties; the template-literal index keeps it cast-free. */
type CssVars = CSSProperties & Record<`--${string}`, string | number>;
const cssVars = (vars: CssVars): CSSProperties => vars;

/** Inline custom properties that paint a theme's roles on one element and its children. */
export const themeVars = (r: ThemeRoles): CSSProperties =>
  cssVars({
    "--tm-bg": r.bg,
    "--tm-panel": r.panel,
    "--tm-line": r.line,
    "--tm-text": r.text,
    "--tm-muted": r.muted,
    "--tm-accent": r.accent,
    "--tm-accent-text": r.accentText,
    "--tm-sel": r.sel,
  });

/** A tiny launcher drawn from a theme's roles: input, tab strip, two rows, status line. */
export function ThemeMini({ roles }: { roles: ThemeRoles }) {
  return (
    <div className="tm" style={themeVars(roles)} aria-hidden="true">
      <div className="tm-in">
        <b>/</b>code
        <span className="tm-caret" />
      </div>
      <div className="tm-tabs">
        <span>/open</span>
        <span>&gt;cmd</span>
        <span>?ask</span>
      </div>
      <div className="tm-row tm-on">
        <span>›</span>Visual Studio Code<em>app</em>
      </div>
      <div className="tm-row">
        <span />
        code-review.md<em>md</em>
      </div>
      <div className="tm-foot">
        <span>list</span>
        <span>enter open</span>
      </div>
    </div>
  );
}
