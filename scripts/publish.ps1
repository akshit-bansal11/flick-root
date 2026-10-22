<#
Publishes a clean snapshot of this private repo (flick-root) to the public repo as one commit.

  pwsh -File scripts/publish.ps1 -Version 0.0.1            # build the snapshot, show it, push nothing
  pwsh -File scripts/publish.ps1 -Version 0.0.1 -Push      # commit "Flick 0.0.1" and push it to public main
  pwsh -File scripts/publish.ps1 -Version 0.0.1 -Push -Tag # also tag v0.0.1, which starts the draft release

The snapshot is HEAD (committed files only) minus the private paths below. It refuses to run with
uncommitted changes, when -Version does not match tauri.conf.json, or when any published file still
mentions a private path. This script is itself private and never published.
#>
param(
    [Parameter(Mandatory)][string]$Version,
    [string]$Remote = 'https://github.com/akshit-bansal11/flick.git',
    [switch]$Push,
    [switch]$Tag
)
$ErrorActionPreference = 'Stop'

# Never published.
$Private = @('CLAUDE.md', 'KICKOFF.md', 'docs/spec', 'docs/followups.md', 'scripts/publish.ps1')
# Lines dropped from published files: biome excludes that name private paths.
$DropLines = @{ 'biome.json' = @('"!.agent",', '"!docs/spec/design-lab.html",') }
# Any of these in a published text file aborts the publish.
$Leaks = 'KICKOFF|CLAUDE\.md|followups\.md|docs/spec|\.agent/|publish\.ps1'

function Invoke-Git { git @args; if ($LASTEXITCODE) { throw "git $($args -join ' ') failed ($LASTEXITCODE)" } }

$root = (git rev-parse --show-toplevel).Trim()
Set-Location $root
if (git status --porcelain --untracked-files=no) { throw 'Uncommitted changes: commit first, the snapshot is HEAD.' }
$confVersion = (Get-Content apps/desktop/src-tauri/tauri.conf.json -Raw | ConvertFrom-Json).version
if ($confVersion -ne $Version) { throw "tauri.conf.json says $confVersion, not $Version." }
$head = (git rev-parse --short HEAD).Trim()

$work = Join-Path ([IO.Path]::GetTempPath()) "flick-publish-$Version"
if (Test-Path $work) { Remove-Item $work -Recurse -Force }
Invoke-Git clone --quiet --depth 1 $Remote $work 2>$null
Get-ChildItem $work -Force | Where-Object Name -ne '.git' | Remove-Item -Recurse -Force

$tarball = Join-Path ([IO.Path]::GetTempPath()) "flick-publish-$Version.tar"
$specs = @('.') + ($Private | ForEach-Object { ":(exclude)$_" })
Invoke-Git archive --format=tar -o $tarball HEAD -- @specs
tar -xf $tarball -C $work
if ($LASTEXITCODE) { throw 'tar failed' }
Remove-Item $tarball

foreach ($p in $Private) { if (Test-Path (Join-Path $work $p)) { throw "Private path leaked: $p" } }
foreach ($file in $DropLines.Keys) {
    $path = Join-Path $work $file
    $kept = Get-Content $path | Where-Object { $DropLines[$file] -notcontains $_.Trim() }
    [IO.File]::WriteAllText($path, (($kept -join "`n") + "`n"))
}

$hits = Get-ChildItem $work -Recurse -File -Force |
    Where-Object { $_.FullName -notmatch '[\\/]\.git[\\/]' -and $_.Extension -notin '.png', '.ico', '.icns', '.woff2', '.lock' } |
    Select-String -Pattern $Leaks
if ($hits) { $hits | ForEach-Object { Write-Host "$($_.Path):$($_.LineNumber): $($_.Line.Trim())" }; throw 'Published files mention private paths (above).' }

Set-Location $work
Invoke-Git add -A
Invoke-Git -c core.safecrlf=false commit --quiet -m "Flick $Version"
Write-Host "Snapshot of $head committed in $work as 'Flick $Version':"
git show --stat --format='%h %s' HEAD | Select-Object -Last 3

if ($Push) {
    Invoke-Git push origin HEAD:main
    if ($Tag) {
        Invoke-Git tag "v$Version"
        Invoke-Git push origin "v$Version"
    }
    Write-Host "Pushed to $Remote."
} else {
    Write-Host 'Dry run: nothing pushed. Inspect the folder above, then rerun with -Push.'
}
