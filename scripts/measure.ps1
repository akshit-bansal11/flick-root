<#
.SYNOPSIS
Measures cold start, hotkey-to-visible latency and idle memory/CPU against the budgets.
.DESCRIPTION
Starts the app (never a second instance), times start to the "ready" log line, sends the
global hotkey with keybd_event, parses hotkey_to_shown_ms from app.log, waits for the process tree (descendants of our PID only) to settle below 1% of one core,
then samples idle RAM and CPU. Stops only the process it started.
Budgets: cold start <= 600 ms, hotkey p95 <= 50 ms, idle RAM <= 120 MB.
Run: pwsh -File scripts/measure.ps1 [-Exe path] [-Toggles 50] [-Hotkey Alt+Space]
.PARAMETER IdleSeconds
Length of the idle CPU averaging window. Default 5.
.PARAMETER Json
If set, writes the metrics (idle CPU, memory, threads, cold start, hotkey samples) to this file.
.PARAMETER Exe
App executable. Default target\release\desktop-shell.exe, else target\debug\desktop-shell.exe.
.PARAMETER Toggles
Number of show+hide pairs.
.PARAMETER Hotkey
Chord such as Alt+Space. Default: general.hotkey from config.toml, else Alt+Space.
#>
[CmdletBinding()]
param(
    [string]$Exe,
    [int]$Toggles = 50,
    [string]$Hotkey,
    [int]$IdleSeconds = 5,
    [string]$Json
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$conf = Get-Content -Raw -LiteralPath (Join-Path $root 'apps\desktop\src-tauri\tauri.conf.json') | ConvertFrom-Json
$app = [string]$conf.productName

if (-not $Exe) {
    # The cargo binary is desktop-shell.exe; productName only names the data folders.
    $Exe = Join-Path $root 'target\release\desktop-shell.exe'
    if (-not (Test-Path -LiteralPath $Exe)) { $Exe = Join-Path $root 'target\debug\desktop-shell.exe' }
}
if (-not (Test-Path -LiteralPath $Exe)) { throw "Executable not found: $Exe" }
Write-Output "Using: $Exe"

$firstRunDone = $false
$cfg = Join-Path $env:APPDATA "$app\config.toml"
$cfgHotkey = $null
if (Test-Path -LiteralPath $cfg) { # read-only; never edited
    $inGeneral = $false
    foreach ($line in Get-Content -LiteralPath $cfg) {
        if ($line -match '^\s*\[(.+)\]\s*$') { $inGeneral = ($Matches[1] -eq 'general') }
        elseif ($inGeneral -and $line -match '^\s*hotkey\s*=\s*"([^"]+)"') { $cfgHotkey = $Matches[1] }
        elseif ($inGeneral -and $line -match '^\s*first_run_done\s*=\s*true') { $firstRunDone = $true }
    }
}
if (-not $Hotkey) { $Hotkey = if ($cfgHotkey) { $cfgHotkey } else { 'Alt+Space' } }
$notComparable = -not $firstRunDone
if ($notComparable) {
    Write-Warning 'First run is not done (general.first_run_done != true): the settings window opens at startup. Idle RAM includes the settings WebView and toggles cannot be captured until first run is completed. RAM and toggles are NOT COMPARABLE.'
}
Write-Output "Hotkey: $Hotkey"

$running = Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq (Resolve-Path -LiteralPath $Exe).Path }
if ($running) { Write-Output 'App already running (single instance); stop it first. Not measuring.'; exit 0 }

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class Keys {
    [DllImport("user32.dll")] static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
    public static void Chord(byte[] vks) {
        foreach (var v in vks) keybd_event(v, 0, 0, UIntPtr.Zero);
        for (int i = vks.Length - 1; i >= 0; i--) keybd_event(vks[i], 0, 2, UIntPtr.Zero);
    }
}
'@

function ConvertTo-VirtualKeys([string]$chord) {
    $vks = foreach ($part in $chord.Split('+')) {
        switch -Regex ($part.Trim()) {
            '^(Alt|Option)$' { 0x12 }
            '^(Ctrl|Control)$' { 0x11 }
            '^Shift$' { 0x10 }
            '^(Win|Super|Meta|Cmd)$' { 0x5B }
            '^Space$' { 0x20 }
            '^[A-Za-z0-9]$' { [int][char]$part.Trim().ToUpperInvariant() }
            '^F([1-9]|1[0-9]|2[0-4])$' { 0x70 + [int]$Matches[1] - 1 }
            default { throw "Unsupported hotkey part: $part" }
        }
    }
    return [byte[]]@($vks)
}
$keys = ConvertTo-VirtualKeys $Hotkey

$logPath = Join-Path $env:LOCALAPPDATA "$app\logs\app.log"
function Read-LogFrom([long]$offset) {
    if (-not (Test-Path -LiteralPath $logPath)) { return '' }
    $fs = [System.IO.File]::Open($logPath, 'Open', 'Read', 'ReadWrite')
    try {
        if ($fs.Length -lt $offset) { $offset = 0 } # rotated
        $null = $fs.Seek($offset, 'Begin')
        return (New-Object System.IO.StreamReader($fs)).ReadToEnd()
    }
    finally { $fs.Dispose() }
}
function Get-LogLength { if (Test-Path -LiteralPath $logPath) { (Get-Item -LiteralPath $logPath).Length } else { 0 } }

function Get-Tree([int]$rootPid) {
    $all = Get-CimInstance Win32_Process
    $ids = [System.Collections.Generic.HashSet[int]]::new()
    $null = $ids.Add($rootPid)
    do {
        $before = $ids.Count
        foreach ($p in $all) { if ($ids.Contains([int]$p.ParentProcessId)) { $null = $ids.Add([int]$p.ProcessId) } }
    } while ($ids.Count -ne $before)
    $cmd = @{}; foreach ($p in $all) { $cmd[[int]$p.ProcessId] = [string]$p.CommandLine }
    foreach ($pr in @(Get-Process -Id @($ids) -ErrorAction SilentlyContinue)) {
        $role = if ($pr.Id -eq $rootPid) { 'app' } elseif ($cmd[$pr.Id] -match '--type=([\w-]+)') { $Matches[1] } elseif ($pr.ProcessName -eq 'msedgewebview2') { 'browser' } else { $pr.ProcessName }
        [pscustomobject]@{ Pid = $pr.Id; Role = $role; WS = [math]::Round($pr.WorkingSet64 / 1MB, 1); Private = [math]::Round($pr.PrivateMemorySize64 / 1MB, 1); Cpu = $pr.CPU }
    }
}

$why = @{
    'cold start ms'    = 'no "ready" log line within 10 s'
    'hotkey p95 ms'    = 'no hotkey_to_shown_ms lines captured'
    'idle RAM MB (ws)' = 'process tree not sampled (app exited or measurement aborted)'
}
$startLen = Get-LogLength
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$proc = Start-Process -FilePath $Exe -PassThru
$coldMs = $null; $p95 = $null; $ws = $null; $settled = $false
try {
    while ($sw.ElapsedMilliseconds -lt 10000) {
        if ((Read-LogFrom $startLen) -match 'phase="ready"') { $coldMs = $sw.Elapsed.TotalMilliseconds; break }
        Start-Sleep -Milliseconds 20
    }
    if ($null -eq $coldMs) { Write-Output 'Cold start: "ready" not seen within 10 s' }

    # Settle: WebView2 bursts CPU for ~25-30 s after launch; wait for the tree to go quiet
    $settleSw = [System.Diagnostics.Stopwatch]::StartNew(); $quiet = 0
    $prev = @{}; foreach ($t in Get-Tree $proc.Id) { $prev[$t.Pid] = $t.Cpu }
    while ($settleSw.Elapsed.TotalSeconds -lt 90) {
        Start-Sleep -Seconds 1
        $delta = 0.0; $cur = @{}
        foreach ($t in Get-Tree $proc.Id) { $cur[$t.Pid] = $t.Cpu; if ($prev.ContainsKey($t.Pid)) { $delta += $t.Cpu - $prev[$t.Pid] } }
        $prev = $cur
        if ($delta -lt 0.01) { $quiet++ } else { $quiet = 0 }
        if ($quiet -ge 5) { $settled = $true; break }
    }
    Write-Output ('Settle: {0} after {1:N0} s' -f $(if ($settled) { 'settled' } else { 'NOT SETTLED' }), $settleSw.Elapsed.TotalSeconds)

    # Toggles
    Start-Sleep -Milliseconds 1000
    $toggleLen = Get-LogLength
    for ($i = 0; $i -lt $Toggles * 2; $i++) {
        [Keys]::Chord($keys)
        Start-Sleep -Milliseconds 300
    }
    Start-Sleep -Milliseconds 500
    $vals = @([regex]::Matches((Read-LogFrom $toggleLen), 'hotkey_to_shown_ms=([0-9.]+)') |
        ForEach-Object { [double]::Parse($_.Groups[1].Value, [cultureinfo]::InvariantCulture) } | Sort-Object)
    $p95 = $null
    if ($vals.Count -gt 0) {
        $p95 = $vals[[Math]::Min($vals.Count - 1, [int][Math]::Ceiling(0.95 * $vals.Count) - 1)]
        $med = $vals[[int][Math]::Floor(($vals.Count - 1) / 2)]
        Write-Output ('Toggles: captured {0}/{1} min {2:N1} median {3:N1} p95 {4:N1} max {5:N1} ms' -f $vals.Count, $Toggles, $vals[0], $med, $p95, $vals[-1])
    }
    else { Write-Output "Toggles: captured 0/$Toggles (hotkey not delivered or focus stolen)" }

    # Idle: launcher should be hidden after an even number of toggles
    Start-Sleep -Seconds 10
    $idleLen = Get-LogLength
    $t0 = @(Get-Tree $proc.Id)
    $idleSw = [System.Diagnostics.Stopwatch]::StartNew()
    Start-Sleep -Seconds $IdleSeconds
    $t1 = @(Get-Tree $proc.Id)
    $idleSecs = $idleSw.Elapsed.TotalSeconds
    $c0 = ($t0 | Measure-Object Cpu -Sum).Sum
    $cpuDelta = if ($t1) { ($t1 | Measure-Object Cpu -Sum).Sum - $c0 } else { $null }
    $cpuPct = if ($t1) { $cpuDelta / $IdleSeconds * 100 / [Environment]::ProcessorCount } else { $null }
    # % of ONE core averaged over the idle window (the enforce gate); processes that exit mid-window can undercount
    $cpuCore = if ($t1) { [math]::Max(0, $cpuDelta) / $idleSecs * 100 } else { $null }
    $threads = (@($t1 | ForEach-Object { (Get-Process -Id $_.Pid -ErrorAction SilentlyContinue).Threads.Count }) | Measure-Object -Sum).Sum
    $idleScans = @((Read-LogFrom $idleLen) -split "`n" | Where-Object { $_ -match 'programs scanned|find index ready|apps refreshed|indexing' })
    $tree = $t1
    $ws = ($tree | Measure-Object WS -Sum).Sum
    $priv = ($tree | Measure-Object Private -Sum).Sum
    $privNoGpu = ($tree | Where-Object Role -ne 'gpu-process' | Measure-Object Private -Sum).Sum
    $tree | Select-Object Pid, Role, WS, Private | Format-Table -AutoSize | Out-String | Write-Output
    Write-Output ('Idle: {0} procs, total WS {1:N0} MB (budget compares this), total private {2:N0} MB, private excl. gpu-process {3:N0} MB, CPU {4:N2}% of machine, {5:N2}% of one core ({6} s), {7} threads, {8} scan log lines' -f $tree.Count, $ws, $priv, $privNoGpu, $cpuPct, $cpuCore, $IdleSeconds, $threads, $idleScans.Count)
    if (-not $settled) { $ws = $null; $why['idle RAM MB (ws)'] = 'not settled within 90 s' }
}
finally {
    if ($proc) {
        $kids = @(Get-Tree $proc.Id | Where-Object Pid -ne $proc.Id | Select-Object -ExpandProperty Pid)
        if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
        Start-Sleep -Seconds 5
        $left = @($kids | Where-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue })
        if ($left) { Write-Output "Lingering child PIDs 5 s after exit (not killed): $($left -join ', ')" }
    }
}

if ($Json) {
    $togglesNote = if ($null -ne $p95) { 'ok' } else { 'inconclusive: no hotkey_to_shown_ms lines captured (keybd_event not delivered on this runner, or focus stolen)' }
    [ordered]@{
        cold_start_ms        = $coldMs
        hotkey_p95_ms        = $p95
        hotkey_samples_ms    = @($vals)
        toggles              = $togglesNote
        settled              = $settled
        idle_seconds         = $IdleSeconds
        idle_cpu_pct_core    = $cpuCore
        idle_cpu_pct_machine = $cpuPct
        working_set_mb       = $(if ($tree) { ($tree | Measure-Object WS -Sum).Sum } else { $null })
        private_mb           = $(if ($tree) { ($tree | Measure-Object Private -Sum).Sum } else { $null })
        thread_count         = $threads
        process_count        = $(if ($tree) { @($tree).Count } else { 0 })
        processes            = @($tree | Select-Object Pid, Role, WS, Private)
        idle_scan_log_lines  = @($idleScans)
        not_comparable       = $notComparable
    } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $Json -Encoding utf8
}

$over = $false
$na = @()
$rows = @(
    [pscustomobject]@{ Metric = 'cold start ms'; Value = $coldMs; Budget = 600; Note = '' }
    [pscustomobject]@{ Metric = 'hotkey p95 ms'; Value = $p95; Budget = 50; Note = $(if ($notComparable) { 'not comparable (first run)' } else { '' }) }
    [pscustomobject]@{ Metric = 'idle RAM MB (ws)'; Value = $ws; Budget = 120; Note = $(if ($notComparable) { 'not comparable (first run)' } else { '' }) }
)
foreach ($r in $rows) {
    if ($null -eq $r.Value) { $na += "$($r.Metric): $($why[$r.Metric])" }
    elseif ($r.Value -gt $r.Budget -and -not $r.Note) { $over = $true }
    $r.Value = if ($null -eq $r.Value) { 'n/a' } else { '{0:N1}' -f $r.Value }
}
$rows | Format-Table -AutoSize | Out-String | Write-Output
foreach ($n in $na) { Write-Output "n/a -> $n" }
if ($over) { exit 1 }
if ($na.Count -gt 0 -or $notComparable) { Write-Output 'Inconclusive.'; exit 2 }
exit 0
