<#
.SYNOPSIS
Cuts a release: bumps versions, regenerates CHANGELOG.md, commits, tags, pushes. Builds nothing.
.DESCRIPTION
The quality gate and the installer build run on GitHub: this script refuses unless HEAD is pushed
and its CI run succeeded. It sets the version in Cargo.toml [workspace.package], tauri.conf.json
and every package.json that has a version, refreshes Cargo.lock (no compile), runs git-cliff, then
commits "up/ release: v<version>", creates an annotated tag and pushes main and the tag. The tag
starts .github/workflows/release.yml, which builds the draft installer.
For a test installer without a release: gh workflow run release.yml, then gh run download <id> -n installer.
Refuses on a dirty tree, a branch other than main, a non-greater version, an existing tag, or a
HEAD that is unpushed or not green in CI.
Never force-pushes. With -DryRun every refusal is reported as WOULD REFUSE and nothing changes.
Run: pwsh -File scripts/release.ps1 <version> [-DryRun] [-NoPush]
.PARAMETER Version
SemVer 2.0.0: x.y.z with optional -prerelease.
.PARAMETER DryRun
Print the plan only.
.PARAMETER NoPush
Commit and tag locally, do not push.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory, Position = 0)][string]$Version,
    [switch]$DryRun,
    [switch]$NoPush
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $root

function Invoke-Native {
    param([string]$Exe, [string[]]$Arguments)
    Write-Host "> $Exe $($Arguments -join ' ')"
    & $Exe @Arguments
    if ($LASTEXITCODE -ne 0) { throw "'$Exe $($Arguments -join ' ')' exited $LASTEXITCODE" }
}
function Get-Native {
    param([string]$Exe, [string[]]$Arguments)
    $out = & $Exe @Arguments
    if ($LASTEXITCODE -ne 0) { throw "'$Exe $($Arguments -join ' ')' exited $LASTEXITCODE" }
    return $out
}

# SemVer 2.0.0 core + optional prerelease (build metadata not accepted for releases).
$semver = '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*)(?:\.(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*))*))?$'
function Test-Greater {
    param([string]$New, [string]$Old)
    $a = [regex]::Match($New, $semver); $b = [regex]::Match($Old, $semver)
    foreach ($i in 1..3) {
        $x = [int64]$a.Groups[$i].Value; $y = [int64]$b.Groups[$i].Value
        if ($x -ne $y) { return $x -gt $y }
    }
    $pa = $a.Groups[4].Value; $pb = $b.Groups[4].Value
    if ($pa -eq $pb) { return $false }
    if ($pa -eq '') { return $true }
    if ($pb -eq '') { return $false }
    $ia = $pa.Split('.'); $ib = $pb.Split('.')
    for ($i = 0; $i -lt [Math]::Min($ia.Count, $ib.Count); $i++) {
        if ($ia[$i] -eq $ib[$i]) { continue }
        $na = $ia[$i] -match '^\d+$'; $nb = $ib[$i] -match '^\d+$'
        if ($na -and $nb) { return [int64]$ia[$i] -gt [int64]$ib[$i] }
        if ($na) { return $false }
        if ($nb) { return $true }
        return [string]::CompareOrdinal($ia[$i], $ib[$i]) -gt 0
    }
    return $ia.Count -gt $ib.Count
}

$problems = [System.Collections.Generic.List[string]]::new()

if ($Version -notmatch $semver) { throw "'$Version' is not SemVer 2.0.0 (x.y.z[-prerelease])." }

$cargoPath = Join-Path $root 'Cargo.toml'
$cargoText = [System.IO.File]::ReadAllText($cargoPath)
$cargoRx = [regex]'(?ms)(\[workspace\.package\][^\[]*?^version\s*=\s*")([^"]+)(")'
$m = $cargoRx.Match($cargoText)
if (-not $m.Success) { throw 'workspace.package version not found in Cargo.toml.' }
$current = $m.Groups[2].Value
if (-not (Test-Greater $Version $current)) { $problems.Add("version $Version is not greater than current $current") }

if (Get-Native git @('status', '--porcelain')) { $problems.Add('working tree is dirty') }
$branch = (Get-Native git @('branch', '--show-current')) -join ''
if ($branch -ne 'main') { $problems.Add("not on main (on '$branch')") }
$tag = "v$Version"
if (Get-Native git @('tag', '--list', $tag)) { $problems.Add("tag $tag exists locally") }
if (Get-Native git @('ls-remote', '--tags', 'origin', "refs/tags/$tag")) { $problems.Add("tag $tag exists on origin") }

# The quality gate runs on GitHub, not here: HEAD must be pushed and green in CI.
$head = (Get-Native git @('rev-parse', 'HEAD')) -join ''
$remote = ((Get-Native git @('ls-remote', 'origin', 'refs/heads/main')) -join '').Split("`t")[0]
if ($head -ne $remote) { $problems.Add('HEAD is not pushed to origin/main (push first so CI can run)') }
$ci = (Get-Native gh @('run', 'list', '--workflow', 'ci.yml', '--commit', $head, '--limit', '1', '--json', 'status,conclusion', '-q', '.[0] | .status + "/" + .conclusion')) -join ''
if ($ci -ne 'completed/success') { $problems.Add("CI for $($head.Substring(0, 7)) is '$(if ($ci) { $ci } else { 'not found' })', not completed/success") }

if ($problems.Count -gt 0) {
    foreach ($p in $problems) { Write-Host "$(if ($DryRun) { 'WOULD REFUSE' } else { 'REFUSED' }): $p" }
    if (-not $DryRun) { exit 1 }
}

# Targets: path + regex replacing the first "version" value (formatting preserved).
$jsonRx = [regex]'(?m)^(\s*"version"\s*:\s*")([^"]+)(")'
$targets = [System.Collections.Generic.List[object]]::new()
$targets.Add(@{ Path = 'Cargo.toml'; Rx = $cargoRx })
$targets.Add(@{ Path = 'apps/desktop/src-tauri/tauri.conf.json'; Rx = $jsonRx })
$pkgs = @('package.json') + @(Get-ChildItem -Path (Join-Path $root 'apps') -Directory | ForEach-Object { "apps/$($_.Name)/package.json" })
foreach ($p in $pkgs) {
    $full = Join-Path $root $p
    if ((Test-Path -LiteralPath $full) -and $jsonRx.IsMatch([System.IO.File]::ReadAllText($full))) {
        $targets.Add(@{ Path = $p; Rx = $jsonRx })
    } else { Write-Host "skip $p (no version field)" }
}

$changed = [System.Collections.Generic.List[string]]::new()
foreach ($t in $targets) {
    $full = Join-Path $root $t.Path
    $text = [System.IO.File]::ReadAllText($full)
    $mm = $t.Rx.Match($text)
    if (-not $mm.Success) { throw "version not found in $($t.Path)" }
    Write-Host "$($t.Path): $($mm.Groups[2].Value) -> $Version"
    $changed.Add($t.Path)
    if (-not $DryRun) {
        $new = $t.Rx.Replace($text, { param($x) $x.Groups[1].Value + $Version + $x.Groups[3].Value }, 1)
        [System.IO.File]::WriteAllText($full, $new, [System.Text.UTF8Encoding]::new($false))
    }
}
$changed.Add('Cargo.lock'); $changed.Add('CHANGELOG.md')

if ($DryRun) {
    Write-Host '> cargo update -w --offline'
    Write-Host "> git-cliff --tag $tag -o CHANGELOG.md"
    Write-Host "> git add $($changed -join ' ')"
    Write-Host "> git commit -m 'up/ release: $tag'"
    Write-Host "> git tag -a $tag -m $tag"
    if (-not $NoPush) { Write-Host "> git push origin main; git push origin $tag" }
    Write-Host 'Dry run: nothing changed.'
    exit 0
}

try {
    & cargo update -w --offline
    if ($LASTEXITCODE -ne 0) {
        Write-Host 'offline cargo update failed; retrying with network.'
        Invoke-Native cargo @('update', '-w')
    }
    Invoke-Native git-cliff @('--tag', $tag, '-o', 'CHANGELOG.md')
} catch {
    Write-Host "FAILED: $_"
    Write-Host 'Changed files (undo with: git checkout -- .):'
    Get-Native git @('status', '--short')
    exit 1
}

Invoke-Native git (@('add', '--') + $changed)
Invoke-Native git (@('commit', '-m', "up/ release: $tag", '--') + $changed)
Invoke-Native git @('tag', '-a', $tag, '-m', $tag)
if (-not $NoPush) {
    Invoke-Native git @('push', 'origin', 'main')
    Invoke-Native git @('push', 'origin', $tag)
}
