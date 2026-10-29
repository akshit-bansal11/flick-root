# Performance

Budgets: hotkey to visible p95 <= 50 ms; command match <= 2 ms for 2,000 apps; each feature query() <= 0.5 ms; Find match p95 <= 25 ms over 300k paths; launch call <= 30 ms; idle RAM <= 120 MB. All rows: date 2026-10-19, dev machine, Windows 11, 24 GB.

| Budget | Limit | Measured | How measured | Build | Status |
| --- | --- | --- | --- | --- | --- |
| Launch call | <= 30 ms | 0-1 ms | live (Calculator, Notepad) | release | within |
| Command match, 2,000 apps | <= 2 ms | 0.25 ms | launcher-core bench | release | within |
| Command search, 2,000 apps, all features on | <= 2 ms | worst 0.29 ms ("2.5 kg to lb"), "code" 0.11 ms, currency 0.06 ms | features bench | release | within |
| Find match p95, 305k paths | <= 25 ms | p95 <= 4.7 ms (<= 3.9 ms with a 2k-entry overlay) | index-files bench | release | within |
| Feature query(): web | <= 0.5 ms | 7.5 us | features bench | release | within |
| Feature query(): syscmd | <= 0.5 ms | <= 2.4 us | features bench | release | within |
| Feature query(): frecency boost | <= 0.5 ms | 0.31 ms per 2k items | features bench (frecency) | release | within |
| Feature query(): math | <= 0.5 ms | 234 us ("2.5 kg to lb") | features bench (math) | release | within (closest) |
| Feature query(): currency, rates loaded | <= 0.5 ms | 2.4 us ("250 usd in inr") | features bench (command_all) | release | within |
| Feature query(): zones | <= 0.5 ms | 1.5 us ("5pm ist in pst") | features bench (math) | release | within |
| Hotkey to visible p95 | <= 50 ms | 241.1 ms (34 of 50 toggles captured, median 61.0, max 242.4) | `scripts/measure.ps1` | release | not comparable (first run not done; script exit 1) |
| Idle RAM | <= 120 MB total incl. WebView2, CPU ~0% | 1,587 MB working set, 976 MB private, 8 processes, CPU 6.19% | `scripts/measure.ps1` | release | over; also not comparable (settings WebView open, first file scan likely still running); under investigation |
| Cold start to tray ready | <= 600 ms | 1,435 ms | `scripts/measure.ps1` | release | over (run included the first-run settings window; under investigation) |
| Keystroke to paint | not yet measured | n/a | n/a | n/a | not yet measured |

### Integrations benches (GitHub `windows-latest` runner, `bench.yml`)

The shared 2-vCPU runner is slower than the laptop above, so compare rows within this table only. Values are criterion means (the middle of the printed interval), not p95.

| Budget | Limit | Measured | Status |
| --- | --- | --- | --- |
| Feature query(): nlfind | <= 0.5 ms | 0.82 us ("pdf from yesterday"), 0.15 us non-natural | within |
| Feature query(): ports | <= 0.5 ms | 0.75 us ("port 3000"), 0.58 us ("proc chrome") | within |
| Feature query(): media | <= 0.5 ms | 0.79 us ("play lofi on spotify") | within |
| Feature query(): capture | <= 0.5 ms | 2.7 us ("log 250 lunch #food"), 0.63 us blocked secret | within |
| Explain secret check, 2,000 chars (not a query()) | n/a | 13.6 us plain, 0.43 us secret found | n/a |
| Command search, 2,000 apps, all features on | <= 2 ms | worst 0.27 ms ("2.5 kg to lb"); "port 3000" 0.09 ms, "log 250 lunch #food" 0.06 ms | within |
| Command match, 2,000 apps (top_k_8) | <= 2 ms | worst 0.22 ms ("co") | within |
| Find match, 305k paths | <= 25 ms p95 | means 13.7–22.1 ms ("rep" with overlay slowest); p95 not reported by criterion | within on means, close on this runner |

### Programs index benches (GitHub `windows-latest` runner, `bench.yml`)

Programs found by the drive scan are matched in their own index, off the keystroke thread, with Find's budget. Criterion means.

| Budget | Limit | Measured | Status |
| --- | --- | --- | --- |
| Command match, 2,000 apps ("!" apps part) | <= 2 ms | 0.08 ms ("code") | within |
| Programs match, 50,000 programs (synthetic names) | <= 25 ms (Find budget) | 0.84–1.61 ms ("c" fastest, "inst 4" slowest) | within |
| Command search, 2,000 apps, all features on | <= 2 ms | worst 0.22 ms; "co" 0.08 ms | within |

### Plain-text search benches (GitHub `windows-latest` runner, `bench.yml`, 2026-10-23)

Plain text with no prefix queries apps, programs and every enabled feature on each keystroke. Means from criterion; a
shared runner is slower and noisier than the dev machine.

| Budget | Limit | Measured | Bench | Status |
| --- | --- | --- | --- | --- |
| All features plus 2,000 apps, per keystroke | <= 2 ms | 53 us ("g cats") to 1.48 ms ("spotfy", typo fallback) | features (command_all) | within |
| Slowest single feature query (math) | <= 0.5 ms | 224 us ("2.5 kg to lb") | features (math) | within |
| Programs, 50,000 paths | none set | 1.04 to 1.45 ms | features (programs_50k) | recorded |
| Find match | p95 <= 25 ms | 12.6 to 21.0 ms (means) | index-files (find) | within, closest to its limit |

## How to reproduce

- Benches run on GitHub only: `gh workflow run bench.yml`, then `gh run download <run-id> -n bench` (bench.txt plus criterion estimates). Bench crates: launcher-core, index-files, features (frecency, math, syscmd, web, command_all, nlfind, ports, media, explain, capture).
- Release exe: `pnpm --filter desktop tauri build --no-bundle` (embeds the built UI), output `target/release/desktop-shell.exe`.
- Live: `pwsh -File scripts/measure.ps1 -Exe target/release/desktop-shell.exe -Toggles 50`. Complete first run first, otherwise the settings window is open at startup and RAM and toggle numbers are not comparable. Do not run it while the app is already running. Exit codes: 0 pass, 1 over budget, 2 inconclusive (a value missing or not comparable).

## Open

- Re-run measure.ps1 after first run is completed. The 2026-10-19 run was done with first_run_done=false, so hotkey p95, idle RAM and cold start are unproven. Idle RAM is the likeliest miss (WebView2 for the launcher alone is large); completing first run now destroys the settings window instead of hiding it.
- Hotkey to visible p95 needs an interactive session with the settings window closed.
- Keystroke to paint has no measurement yet.

## WebView2 idle memory

Researched 2026-10-22. No documented option works without a new dependency, so Flick depends directly on `webview2-com` (the version wry already uses). The two documented memory APIs now run while the launcher is hidden; the numbers are below.

| Option | Expected saving | Show-latency cost / risk | Status |
| --- | --- | --- | --- |
| `TrySuspend` / `Resume` ([perf guide](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/performance#use-memory-management-apis)) | Pauses scripts, lowers renderer use while hidden | Resume on every show sits on the hotkey path; best-effort | **shipped** (uses `webview2-com` 0.39.1; the controller is set invisible first, as [TrySuspend]((https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2_3#trysuspend)) requires) |
| `MemoryUsageTargetLevel = Low` while hidden, `Normal` on show (same page) | Engine may drop caches or swap to disk | Restore on show may add latency; best-effort | **shipped** |
| `additional_browser_args` ([docs.rs](https://docs.rs/tauri/latest/tauri/webview/struct.WebviewBuilder.html#method.additional_browser_args)) | None documented | Replaces wry's default `--disable-features` set; needs the window built in code instead of `tauri.conf.json`; risk to the transparent window | not shipped: [flag list](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/webview-features-flags) says production apps should not use flags, and none is a memory saver |
| `msWebView2SimulateMemoryPressureWhenInactive` | Unknown ("simulates memory pressure for an inactive WebView") | Unknown | not shipped (undocumented effect and behaviour) |
| `disable-background-timer-throttling` | None (the opposite: more timer work) | More idle CPU | not shipped (wrong direction) |
| `disable-gpu` | Likely less GPU memory | Docs say not to use it outside troubleshooting; breaks acrylic/transparent rendering risk | not shipped |
| Renderer process limits, other memory-saving switches | n/a | n/a | not shipped (undocumented) |
| Tauri `background_throttling` | n/a | Not supported on Windows | not applicable |

### Measured (perf.yml, GitHub `windows-latest` runner, 2026-10-22)

Process tree = the app plus its WebView2 processes (7 processes, 128 threads in every run). Idle CPU was 0 % of one core and no scan ran during the 60 s idle window in every run.

| Build | `perf.yml` run | Hotkey p95 (ms) | Idle private MB | Idle working set MB | Cold start (ms) |
| --- | --- | --- | --- | --- | --- |
| Before (no WebView2 change) | 1 | 148.4 | 111.8 | 326.7 | 728 |
| Before | 2 | 136.6 | 111.6 | 327.1 | 1134 |
| Low memory level only | 1 | 172.4 | 112.0 | 83.4 | 1376 |
| Low memory level only | 2 | 128.1 | 113.7 | 75.5 | 909 |
| Low level + TrySuspend | 1 | 181.4 | 109.8 | 64.2 | 813 |

Reading: the working set (physical RAM in use while hidden) drops from about 327 MB to 64 MB. Private bytes stay flat because suspended pages are released from RAM, not freed. Timings swing by hundreds of milliseconds between identical runs on this shared VM (cold start 728 vs 1134 ms for the same commit), so the hotkey p95 differences are within noise; the hotkey is over its 50 ms budget on the runner with or without the change and needs a check on real hardware.
