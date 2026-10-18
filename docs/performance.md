# Performance

Budgets are from KICKOFF section 4. All rows: date 2026-10-19, dev machine, Windows 11, 24 GB.

| Budget | Limit | Measured | How measured | Build | Status |
| --- | --- | --- | --- | --- | --- |
| Launch call | <= 30 ms | 0-1 ms | live (Calculator, Notepad) | release | within |
| Command match, 2,000 apps | <= 2 ms | 0.25 ms | launcher-core bench | release | within |
| Command search, 2,000 apps, all v0.0.1 features on | <= 2 ms | worst 0.29 ms ("2.5 kg to lb"), "code" 0.11 ms, currency 0.06 ms | features bench (T5.6) | release | within |
| Find match p95, 305k paths | <= 25 ms | p95 <= 4.7 ms (<= 3.9 ms with a 2k-entry overlay) | index-files bench | release | within |
| Feature query(): web | <= 0.5 ms | 7.5 us | features bench | release | within |
| Feature query(): syscmd | <= 0.5 ms | <= 2.4 us | features bench | release | within |
| Feature query(): frecency boost | <= 0.5 ms | 0.31 ms per 2k items | features bench (frecency) | release | within |
| Feature query(): math | <= 0.5 ms | 234 us ("2.5 kg to lb") | features bench (math) | release | within (closest) |
| Feature query(): currency, rates loaded | <= 0.5 ms | 2.4 us ("250 usd in inr") | features bench (command_all) | release | within |
| Feature query(): projects | <= 0.5 ms | 51 us per 500 projects | features bench (projects) | release | within |
| Feature query(): zones | <= 0.5 ms | 1.5 us ("5pm ist in pst") | features bench (math) | release | within |
| Hotkey to visible p95 | <= 50 ms | 241.1 ms (34 of 50 toggles captured, median 61.0, max 242.4) | `scripts/measure.ps1` | release | not comparable (first run not done; script exit 1) |
| Idle RAM | <= 120 MB total incl. WebView2, CPU ~0% | 1,587 MB working set, 976 MB private, 8 processes, CPU 6.19% | `scripts/measure.ps1` | release | over; also not comparable (settings WebView open, first file scan likely still running); under investigation |
| Cold start to tray ready | <= 600 ms | 1,435 ms | `scripts/measure.ps1` | release | over (run included the first-run settings window; under investigation) |
| Keystroke to paint | not yet measured | n/a | n/a | n/a | not yet measured |

## How to reproduce

- Benches: `cargo bench -p launcher-core`, `cargo bench -p index-files`, `cargo bench -p features` (frecency, math, syscmd, web, projects, command_all).
- Release exe: `pnpm --filter desktop tauri build --no-bundle` (embeds the built UI), output `target/release/desktop-shell.exe`.
- Live: `pwsh -File scripts/measure.ps1 -Exe target/release/desktop-shell.exe -Toggles 50`. Complete first run first, otherwise the settings window is open at startup and RAM and toggle numbers are not comparable. Do not run it while the app is already running. Exit codes: 0 pass, 1 over budget, 2 inconclusive (a value missing or not comparable).

## Open

- Re-run measure.ps1 after first run is completed. The 2026-10-19 run was done with first_run_done=false, so hotkey p95, idle RAM and cold start are unproven. Idle RAM is the likeliest miss (WebView2 for the launcher alone is large); completing first run now destroys the settings window instead of hiding it.
- Hotkey to visible p95 needs an interactive session with the settings window closed.
- Keystroke to paint has no measurement yet.
