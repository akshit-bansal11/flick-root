import { type KeyboardEvent, useEffect, useRef, useState } from "react";
import { comboFromEvent, validateCombo } from "../firstrun/hotkeyRules";
import * as api from "./api";
import "./features.css";
import { OnOff } from "./OnOff";
import { SettingRow } from "./Row";
import { errorText, type GeneralPatch, type SettingsView } from "./types";

interface Props {
  settings: SettingsView;
  onChange: (s: SettingsView) => void;
  onTour?: () => void;
}

const DRIVE_RE = /^[A-Za-z]:\\?$/;
const sameDrive = (a: string, b: string) =>
  DRIVE_RE.test(a) && a.charAt(0).toUpperCase() === b.charAt(0).toUpperCase();

export function General({ settings, onChange, onTour }: Props) {
  const [errors, setErrors] = useState<Record<string, string | null>>({});
  const [capturing, setCapturing] = useState(false);
  const captureRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (capturing) captureRef.current?.focus();
  }, [capturing]);
  const [rootInput, setRootInput] = useState("");
  const [skipInput, setSkipInput] = useState("");
  const [editing, setEditing] = useState<"skip" | "roots" | null>(null);
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

  const fail = (k: string) =>
    errors[k] && (
      <div className="s-error" role="alert">
        {errors[k]}
      </div>
    );
  const toggleEdit = (k: "skip" | "roots") => setEditing(editing === k ? null : k);
  const skipFolders = settings.programSkip.filter((s) => !DRIVE_RE.test(s));
  const programsNote = `${settings.programsCount} found.${settings.programsCapped ? " A drive hit its limit, so some programs are missing." : ""}`;

  return (
    <section>
      <h1 className="s-title">General</h1>
      <p className="s-desc">Hotkey and where Find looks.</p>

      <h2 className="sec"># launcher</h2>
      <SettingRow
        label="hotkey"
        value={settings.hotkeyLauncher}
        preview={{ title: "# hotkey", body: "Opens the launcher from anywhere." }}
      >
        {capturing ? (
          <button
            type="button"
            className="btn"
            ref={captureRef}
            onKeyDown={onCapture}
            onBlur={() => setCapturing(false)}
          >
            Press keys… (Esc cancels)
          </button>
        ) : (
          <button type="button" className="btn" onClick={() => setCapturing(true)}>
            change
          </button>
        )}
      </SettingRow>
      {fail("hotkey")}
      <SettingRow
        label="launch at startup"
        preview={{ title: "# launch at startup", body: "Starts the launcher when you sign in." }}
      >
        <OnOff
          label="launch at startup"
          checked={settings.launchAtStartup}
          onChange={(on) => void patch("startup", { launchAtStartup: on })}
        />
      </SettingRow>
      {fail("startup")}
      <SettingRow
        label="check for updates"
        hint="daily, asks GitHub only"
        preview={{
          title: "# check for updates",
          body: "Once a day, asks GitHub whether a newer release exists. Nothing else is sent.",
        }}
      >
        <OnOff
          label="check for updates"
          checked={settings.checkUpdates}
          onChange={(on) => void patch("updates", { checkUpdates: on })}
        />
      </SettingRow>
      {fail("updates")}

      <h2 className="sec"># where find looks</h2>
      <SettingRow
        label="index all drives"
        hint="D:, E: · system folders skipped"
        preview={{
          title: "# index all drives",
          body: "Fixed, non-system drives. System folders like Windows and Program Files are skipped.",
        }}
      >
        <OnOff
          label="index all drives"
          checked={settings.includeDrives}
          onChange={(on) => void toggleDrives(on)}
        />
      </SettingRow>
      {fail("drives")}
      <SettingRow
        label="find programs on all drives"
        hint={programsNote}
        preview={{
          title: "# find programs on all drives",
          body: `Scans every fixed and removable drive for exe, msi, apk and other installers. Network and optical drives are never touched. ${programsNote}`,
        }}
      >
        <OnOff
          label="find programs on all drives"
          checked={settings.scanPrograms}
          onChange={(on) => void patch("programs", { scanPrograms: on })}
        />
      </SettingRow>
      {fail("programs")}
      <SettingRow
        label="drives"
        hint="untick a drive to skip it"
        preview={{
          title: "# drives",
          body: "Untick a drive and the program scan skips it. Needs program scanning on.",
        }}
      />
      <ul className="s-list s-editor">
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
      {fail("skipDrives")}
      <SettingRow
        label="excluded folders"
        value={skipFolders.join("  ")}
        hint="none"
        preview={{
          title: "# excluded folders",
          body: "A folder (D:\\Games), a drive (E:\\) or a folder name (node_modules, .*)",
        }}
      >
        <button
          type="button"
          className="btn"
          aria-label="Edit excluded folders"
          aria-expanded={editing === "skip"}
          onClick={() => toggleEdit("skip")}
        >
          edit
        </button>
      </SettingRow>
      {editing === "skip" && (
        <div className="s-roots s-editor">
          <ul className="s-list">
            {skipFolders.map((s) => (
              <li key={s} className="s-list-item">
                <span className="s-path">{s}</span>
                <button
                  type="button"
                  className="btn"
                  aria-label={`Remove ${s}`}
                  disabled={!settings.scanPrograms}
                  onClick={() => void editSkip("skip", settings.programSkip.filter((x) => x !== s))}
                >
                  remove
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
              className="btn"
              disabled={!settings.scanPrograms}
              onClick={() => void addSkip()}
            >
              add
            </button>
          </div>
        </div>
      )}
      {fail("skip")}
      <SettingRow
        label="indexed folders"
        value={settings.extraRoots.join("  ")}
        hint="none"
        preview={{ title: "# indexed folders", body: "Extra folders Open searches." }}
      >
        <button
          type="button"
          className="btn"
          aria-label="Edit indexed folders"
          aria-expanded={editing === "roots"}
          onClick={() => toggleEdit("roots")}
        >
          edit
        </button>
      </SettingRow>
      {editing === "roots" && (
        <div className="s-roots s-editor">
          <ul className="s-list">
            {settings.extraRoots.map((r) => (
              <li key={r} className="s-list-item">
                <span className="s-path">{r}</span>
                <button
                  type="button"
                  className="btn"
                  aria-label={`Remove ${r}`}
                  onClick={() =>
                    void run("roots", api.setRoots(settings.extraRoots.filter((x) => x !== r)))
                  }
                >
                  remove
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
            <button type="button" className="btn" onClick={() => void addRoot()}>
              add
            </button>
          </div>
        </div>
      )}
      {fail("roots")}

      <h2 className="sec"># maintenance</h2>
      <SettingRow
        label="history"
        hint={cleared ? "history cleared" : "forgets which results you picked"}
        preview={{ title: "# history", body: "Forgets which results you picked." }}
      >
        {confirming ? (
          <span className="s-add">
            <button type="button" className="btn s-btn-danger" onClick={() => void clear()}>
              confirm clear
            </button>
            <button type="button" className="btn" onClick={() => setConfirming(false)}>
              cancel
            </button>
          </span>
        ) : (
          <button
            type="button"
            className="btn"
            onClick={() => {
              setCleared(false);
              setConfirming(true);
            }}
          >
            clear
          </button>
        )}
      </SettingRow>
      {fail("history")}
      <SettingRow
        label="indexes"
        hint="rebuild apps and files"
        preview={{ title: "# indexes", body: "Rebuild the app and file indexes." }}
      >
        <button type="button" className="btn" disabled={reindexing} onClick={() => void reindex()}>
          {reindexing ? "reindexing…" : "reindex"}
        </button>
      </SettingRow>
      {fail("reindex")}
      <SettingRow
        label="tour"
        hint="a 2-minute tour of every mode"
        preview={{ title: "# tour", body: "A 2-minute interactive tour of every mode." }}
      >
        <button type="button" className="btn" onClick={onTour}>
          take the tour
        </button>
      </SettingRow>
    </section>
  );
}
