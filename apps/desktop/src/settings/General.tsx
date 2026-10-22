import { type KeyboardEvent, type ReactNode, useEffect, useRef, useState } from "react";
import { comboFromEvent, validateCombo } from "../firstrun/hotkeyRules";
import * as api from "./api";
import { Toggle } from "./Toggle";
import { errorText, type GeneralPatch, type SettingsView } from "./types";

interface Props {
  settings: SettingsView;
  onChange: (s: SettingsView) => void;
  onTour?: () => void;
}

const DRIVE_RE = /^[A-Za-z]:\\?$/;
const sameDrive = (a: string, b: string) =>
  DRIVE_RE.test(a) && a.charAt(0).toUpperCase() === b.charAt(0).toUpperCase();

function Row({
  label,
  help,
  error,
  children,
}: {
  label: string;
  help?: string;
  error?: string | null;
  children: ReactNode;
}) {
  return (
    <div className="s-row">
      <div className="s-row-text">
        <div className="s-row-label">{label}</div>
        {help && <div className="s-help">{help}</div>}
        {error && (
          <div className="s-error" role="alert">
            {error}
          </div>
        )}
      </div>
      <div className="s-row-control">{children}</div>
    </div>
  );
}

export function General({ settings, onChange, onTour }: Props) {
  const [errors, setErrors] = useState<Record<string, string | null>>({});
  const [capturing, setCapturing] = useState(false);
  const captureRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (capturing) captureRef.current?.focus();
  }, [capturing]);
  const [rootInput, setRootInput] = useState("");
  const [skipInput, setSkipInput] = useState("");
  const [confirming, setConfirming] = useState(false);
  const [cleared, setCleared] = useState(false);
  const [reindexing, setReindexing] = useState(false);

  const setError = (k: string, e: unknown) =>
    setErrors((p) => ({ ...p, [k]: e ? errorText(e) : null }));

  const run = async (k: string, p: Promise<SettingsView>): Promise<boolean> => {
    try {
      onChange(await p);
      setError(k, null);
      return true;
    } catch (e) {
      setError(k, e);
      return false;
    }
  };
  const patch = (k: string, g: GeneralPatch) => run(k, api.setGeneral(g));

  const onCapture = (e: KeyboardEvent) => {
    e.preventDefault();
    // The window handler must not see these as Esc / Ctrl+W / Ctrl+Tab.
    e.stopPropagation();
    if (e.key === "Escape") return setCapturing(false);
    const c = comboFromEvent(e);
    if (!c) return;
    setCapturing(false);
    const bad = validateCombo(c, settings.hotkeyExplain);
    if (bad) return setError("hotkey", bad);
    void patch("hotkey", { hotkeyLauncher: c });
  };

  const addRoot = async () => {
    const path = rootInput.trim();
    if (!path || settings.extraRoots.includes(path)) return;
    if (await run("roots", api.setRoots([...settings.extraRoots, path]))) setRootInput("");
  };

  const toggleDrives = async (on: boolean) => {
    onChange({ ...settings, includeDrives: on });
    if (!(await run("drives", api.setIncludeDrives(on)))) onChange(settings);
  };

  const editSkip = async (k: string, next: string[]): Promise<boolean> => {
    onChange({ ...settings, programSkip: next });
    const ok = await run(k, api.setProgramSkip(next));
    if (!ok) onChange(settings);
    return ok;
  };

  const addSkip = async () => {
    const entry = skipInput.trim();
    if (!entry || settings.programSkip.includes(entry)) return;
    if (await editSkip("skip", [...settings.programSkip, entry])) setSkipInput("");
  };

  const clear = async () => {
    try {
      await api.clearHistory("all");
      setError("history", null);
      setCleared(true);
    } catch (e) {
      setError("history", e);
    }
    setConfirming(false);
  };

  const reindex = async () => {
    setReindexing(true);
    try {
      await api.reindex("apps");
      if (settings.findEnabled) await api.reindex("files");
      setError("reindex", null);
    } catch (e) {
      setError("reindex", e);
    }
    setReindexing(false);
  };

  return (
    <section>
      <h1 className="s-title">General</h1>
      <p className="s-desc">Hotkey and where Find looks.</p>

      <Row label="Hotkey" help="Opens the launcher from anywhere." error={errors.hotkey}>
        <kbd className="s-kbd">{settings.hotkeyLauncher}</kbd>
        {capturing ? (
          <button
            type="button"
            className="s-btn"
            ref={captureRef}
            onKeyDown={onCapture}
            onBlur={() => setCapturing(false)}
          >
            Press keys… (Esc cancels)
          </button>
        ) : (
          <button type="button" className="s-btn" onClick={() => setCapturing(true)}>
            Change
          </button>
        )}
      </Row>

      <Row label="Launch at startup" error={errors.startup}>
        <Toggle
          label="Launch at startup"
          checked={settings.launchAtStartup}
          onChange={(on) => void patch("startup", { launchAtStartup: on })}
        />
      </Row>

      <Row
        label="Check for updates"
        help="Once a day, asks GitHub whether a newer release exists. Nothing else is sent."
        error={errors.updates}
      >
        <Toggle
          label="Check for updates"
          checked={settings.checkUpdates}
          onChange={(on) => void patch("updates", { checkUpdates: on })}
        />
      </Row>

      <Row
        label="Index all drives (D:, E: …)"
        help="Fixed, non-system drives. System folders like Windows and Program Files are skipped."
        error={errors.drives}
      >
        <Toggle
          label="Index all drives"
          checked={settings.includeDrives}
          onChange={(on) => void toggleDrives(on)}
        />
      </Row>

      <Row
        label="Find programs on all drives"
        help={`Scans every fixed and removable drive for exe, msi, apk and other installers. Network and optical drives are never touched. ${settings.programsCount} found.${settings.programsCapped ? " A drive hit its limit, so some programs are missing." : ""}`}
        error={errors.programs}
      >
        <Toggle
          label="Find programs on all drives"
          checked={settings.scanPrograms}
          onChange={(on) => void patch("programs", { scanPrograms: on })}
        />
      </Row>

      <Row label="Drives" error={errors.skipDrives}>
        <ul className="s-list">
          {settings.drives.map((d) => (
            <li key={d.root} className="s-list-item">
              <label className="s-path">
                <input
                  type="checkbox"
                  disabled={!settings.scanPrograms}
                  checked={!settings.programSkip.some((s) => sameDrive(s, d.root))}
                  onChange={(e) =>
                    void editSkip(
                      "skipDrives",
                      e.target.checked
                        ? settings.programSkip.filter((s) => !sameDrive(s, d.root))
                        : [...settings.programSkip, d.root],
                    )
                  }
                />{" "}
                {d.root} · {d.kind}
                {d.present ? "" : " (not connected)"}
              </label>
            </li>
          ))}
        </ul>
      </Row>

      <Row
        label="Excluded folders"
        help="A folder (D:\Games), a drive (E:\) or a folder name (node_modules, .*)"
        error={errors.skip}
      >
        <div className="s-roots">
          <ul className="s-list">
            {settings.programSkip
              .filter((s) => !DRIVE_RE.test(s))
              .map((s) => (
                <li key={s} className="s-list-item">
                  <span className="s-path">{s}</span>
                  <button
                    type="button"
                    className="s-btn"
                    aria-label={`Remove ${s}`}
                    disabled={!settings.scanPrograms}
                    onClick={() =>
                      void editSkip(
                        "skip",
                        settings.programSkip.filter((x) => x !== s),
                      )
                    }
                  >
                    Remove
                  </button>
                </li>
              ))}
          </ul>
          <div className="s-add">
            <input
              className="s-input"
              aria-label="Exclude"
              placeholder="D:\Games"
              disabled={!settings.scanPrograms}
              value={skipInput}
              onChange={(e) => setSkipInput(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && void addSkip()}
            />
            <button
              type="button"
              className="s-btn"
              disabled={!settings.scanPrograms}
              onClick={() => void addSkip()}
            >
              Add
            </button>
          </div>
        </div>
      </Row>

      <Row label="Indexed folders" help="Extra folders Find mode searches." error={errors.roots}>
        <div className="s-roots">
          <ul className="s-list">
            {settings.extraRoots.map((r) => (
              <li key={r} className="s-list-item">
                <span className="s-path">{r}</span>
                <button
                  type="button"
                  className="s-btn"
                  aria-label={`Remove ${r}`}
                  onClick={() =>
                    void run("roots", api.setRoots(settings.extraRoots.filter((x) => x !== r)))
                  }
                >
                  Remove
                </button>
              </li>
            ))}
          </ul>
          <div className="s-add">
            <input
              className="s-input"
              aria-label="Folder path"
              placeholder="C:\Projects"
              value={rootInput}
              onChange={(e) => setRootInput(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && void addRoot()}
            />
            <button type="button" className="s-btn" onClick={() => void addRoot()}>
              Add
            </button>
          </div>
        </div>
      </Row>

      <Row
        label="Clear history"
        help={cleared ? "History cleared." : "Forgets which results you picked."}
        error={errors.history}
      >
        {confirming ? (
          <>
            <button type="button" className="s-btn s-btn-danger" onClick={() => void clear()}>
              Confirm clear
            </button>
            <button type="button" className="s-btn" onClick={() => setConfirming(false)}>
              Cancel
            </button>
          </>
        ) : (
          <button
            type="button"
            className="s-btn"
            onClick={() => {
              setCleared(false);
              setConfirming(true);
            }}
          >
            Clear history
          </button>
        )}
      </Row>

      <Row label="Reindex" help="Rebuild the app and file indexes." error={errors.reindex}>
        <button
          type="button"
          className="s-btn"
          disabled={reindexing}
          onClick={() => void reindex()}
        >
          {reindexing ? "Reindexing…" : "Reindex"}
        </button>
      </Row>

      <Row label="Tour" help="A 2-minute interactive tour of every mode.">
        <button type="button" className="s-btn" onClick={onTour}>
          Take the tour
        </button>
      </Row>
    </section>
  );
}
