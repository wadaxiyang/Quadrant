# SPDX-FileCopyrightText: Copyright (c) 2026 Quadrant contributors
# SPDX-License-Identifier: GPL-3.0-only

[CmdletBinding()]
param(
    [ValidateSet('Smoke', 'Matrix', 'All')]
    [string]$Mode = 'Smoke',
    [string]$OutputDirectory,
    [switch]$ReuseExisting
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $repoRoot 'docs/visual-baselines/ui-gallery-stage0'
}
$resolvedOutput = [System.IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $resolvedOutput | Out-Null

$saved = @{}
foreach ($name in @('QUADRANT_GALLERY_WIDTH', 'QUADRANT_GALLERY_HEIGHT', 'QUADRANT_GALLERY_THEME', 'QUADRANT_GALLERY_SNAPSHOT', 'SLINT_SCALE_FACTOR')) {
    $saved[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}

$records = [System.Collections.Generic.List[object]]::new()
$catalog = [System.Collections.Generic.List[object]]::new()
$catalog.Add([pscustomobject]@{
    width = 1040
    height = 800
    theme = 'light'
    scale_percent = 100
    file = 'gallery-smoke-1040x800-light-100.png'
})
foreach ($size in @(@(760, 520), @(900, 600), @(1100, 720), @(1440, 900))) {
    foreach ($theme in @('light', 'dark')) {
        foreach ($scale in @(100, 125, 150, 200)) {
            $catalog.Add([pscustomobject]@{
                width = $size[0]
                height = $size[1]
                theme = $theme
                scale_percent = $scale
                file = "gallery-$($size[0])x$($size[1])-$theme-$scale.png"
            })
        }
    }
}

function Invoke-GallerySnapshot {
    param(
        [int]$Width,
        [int]$Height,
        [ValidateSet('light', 'dark')]
        [string]$Theme,
        [int]$ScalePercent,
        [string]$FileName
    )

    $snapshotPath = Join-Path $resolvedOutput $FileName
    if ($ReuseExisting -and (Test-Path -LiteralPath $snapshotPath)) {
        Write-Host "Reusing $FileName"
    }
    else {
        $env:QUADRANT_GALLERY_WIDTH = [string]$Width
        $env:QUADRANT_GALLERY_HEIGHT = [string]$Height
        $env:QUADRANT_GALLERY_THEME = $Theme
        $env:SLINT_SCALE_FACTOR = [string]::Format([Globalization.CultureInfo]::InvariantCulture, '{0:0.##}', $ScalePercent / 100.0)
        $env:QUADRANT_GALLERY_SNAPSHOT = $snapshotPath

        Write-Host "Capturing $FileName"
        & cargo run --manifest-path (Join-Path $repoRoot 'Cargo.toml') --locked --quiet -p quadrant-ui-gallery
        if ($LASTEXITCODE -ne 0) {
            throw "Gallery snapshot command failed for $FileName with exit code $LASTEXITCODE"
        }
    }
    $file = Get-Item -LiteralPath $snapshotPath
    if ($file.Length -le 0) {
        throw "Gallery snapshot is empty: $snapshotPath"
    }
}

try {
    foreach ($cell in $catalog) {
        $isSmoke = $cell.file.StartsWith('gallery-smoke-')
        $selected = $Mode -eq 'All' -or ($Mode -eq 'Smoke' -and $isSmoke) -or ($Mode -eq 'Matrix' -and -not $isSmoke)
        if ($selected) {
            Invoke-GallerySnapshot -Width $cell.width -Height $cell.height -Theme $cell.theme -ScalePercent $cell.scale_percent -FileName $cell.file
        }
    }

    foreach ($cell in $catalog) {
        $snapshotPath = Join-Path $resolvedOutput $cell.file
        if (Test-Path -LiteralPath $snapshotPath) {
            $file = Get-Item -LiteralPath $snapshotPath
            if ($file.Length -le 0) {
                throw "Gallery snapshot is empty: $snapshotPath"
            }
            $records.Add([pscustomobject]@{
                file = $cell.file
                width = $cell.width
                height = $cell.height
                theme = $cell.theme
                scale_percent = $cell.scale_percent
                bytes = $file.Length
                sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $snapshotPath).Hash.ToLowerInvariant()
            })
        }
    }

    $manifestPath = Join-Path $resolvedOutput 'manifest.json'
    $manifest = [ordered]@{
        schema_version = 1
        baseline = 'Quadrant UI Gallery Stage 0'
        source_commit = (& git -C $repoRoot rev-parse --short HEAD).Trim()
        generated_at_utc = [DateTime]::UtcNow.ToString('o')
        mode = $Mode
        snapshots = @($records)
    }
    $manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $manifestPath -Encoding utf8
    Write-Host "Wrote $($records.Count) snapshot record(s) to $manifestPath"
}
finally {
    foreach ($name in $saved.Keys) {
        [Environment]::SetEnvironmentVariable($name, $saved[$name], 'Process')
    }
}
