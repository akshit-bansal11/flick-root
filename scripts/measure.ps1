<#
.SYNOPSIS
Measures cold start, hotkey-to-visible latency and idle memory/CPU against the budgets.
.DESCRIPTION
Starts the app (never a second instance), times start to the "ready" log line, sends the
global hotkey with SendInput, parses hotkey_to_shown_ms from app.log, then samples idle
RAM (app plus msedgewebview2 children) and CPU. Stops only the process it started.
Budgets: cold start <= 600 ms, hotkey p95 <= 50 ms, idle RAM <= 120 MB.
Run: pwsh -File scripts/measure.ps1 [-Exe path] [-Toggles 50] [-Hotkey Alt+Space]
.PARAMETER Exe
App executable. Default target\release\<productName>.exe, else target\debug\desktop-shell.exe.
.PARAMETER Toggles
Number of show+hide pairs.
.PARAMETER Hotkey
Chord such as Alt+Space. Default: general.hotkey from config.toml, else Alt+Space.
#>
[CmdletBinding()]
param(
    [string]$Exe,
    [int]$Toggles = 50,
    [string]$Hotkey
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$conf = Get-Content -Raw -LiteralPath (Join-Path $root 'apps\desktop\src-tauri\tauri.conf.json') | ConvertFrom-Json
$app = [string]$conf.productName

if (-not $Exe) {
    $Exe = Join-Path $root "target\release\$app.exe"
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

$startLen = Get-LogLength
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$proc = Start-Process -FilePath $Exe -PassThru
$coldMs = $null; $p95 = $null; $ws = $null
try {
    while ($sw.ElapsedMilliseconds -lt 10000) {
        if ((Read-LogFrom $startLen) -match '(?m)\bready\s*$') { $coldMs = $sw.Elapsed.TotalMilliseconds; break }
        Start-Sleep -Milliseconds 20
    }
    if ($null -eq $coldMs) { Write-Output 'Cold start: "ready" not seen within 10 s' }

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
    $all = Get-CimInstance Win32_Process
    $ids = [System.Collections.Generic.HashSet[int]]::new()
    $null = $ids.Add($proc.Id)
    do {
        $before = $ids.Count
        foreach ($p in $all) { if ($ids.Contains([int]$p.ParentProcessId)) { $null = $ids.Add([int]$p.ProcessId) } }
    } while ($ids.Count -ne $before)
    $tree = @(Get-Process -Id @($ids) -ErrorAction SilentlyContinue)
    $ws = ($tree | Measure-Object WorkingSet64 -Sum).Sum / 1MB
    $priv = ($tree | Measure-Object PrivateMemorySize64 -Sum).Sum / 1MB
    $cpu0 = ($tree | Measure-Object CPU -Sum).Sum
    Start-Sleep -Seconds 5
    $tree = @(Get-Process -Id @($ids) -ErrorAction SilentlyContinue)
    $cpuPct = (($tree | Measure-Object CPU -Sum).Sum - $cpu0) / 5 * 100 / [Environment]::ProcessorCount
    Write-Output ('Idle: {0} procs, working set {1:N0} MB, private {2:N0} MB, CPU {3:N2}%' -f $tree.Count, $ws, $priv, $cpuPct)
}
finally {
    if ($proc -and -not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
}

$over = $false
$na = @()
$why = @{
    'cold start ms'    = 'no "ready" log line within 10 s'
    'hotkey p95 ms'    = 'no hotkey_to_shown_ms lines captured'
    'idle RAM MB (ws)' = 'process tree not sampled (app exited or measurement aborted)'
}
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
