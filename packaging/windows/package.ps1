param(
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
$repository = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..')).Path
$targetRoot = Join-Path $repository 'target\package\windows'
$versionLine = Get-Content -LiteralPath (Join-Path $repository 'Cargo.toml') |
    Where-Object { $_ -match '^version = "([^"]+)"' } |
    Select-Object -First 1
if ($versionLine -notmatch '^version = "([^"]+)"') {
    throw 'Workspace package version was not found.'
}
$version = $Matches[1]

if (-not $SkipBuild) {
    $env:QUADRANT_DISTRIBUTION_CHANNEL = 'windows-portable'
    & cargo build --manifest-path (Join-Path $repository 'Cargo.toml') --locked --release -p quadrant-app -p quadrant-agent
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed with exit code $LASTEXITCODE" }
}

$staging = Join-Path $targetRoot "Quadrant-$version-windows-x86_64"
$resolvedTarget = [System.IO.Path]::GetFullPath($targetRoot)
$resolvedStaging = [System.IO.Path]::GetFullPath($staging)
if (-not $resolvedStaging.StartsWith($resolvedTarget + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw 'Refusing to replace a staging directory outside target/package/windows.'
}
if (Test-Path -LiteralPath $staging) {
    Remove-Item -LiteralPath $staging -Recurse -Force
}
New-Item -ItemType Directory -Path $staging | Out-Null

Copy-Item -LiteralPath (Join-Path $repository 'target\release\quadrant-app.exe') -Destination (Join-Path $staging 'quadrant.exe')
Copy-Item -LiteralPath (Join-Path $repository 'target\release\quadrant-agent.exe') -Destination $staging
Copy-Item -LiteralPath (Join-Path $repository 'LICENSE') -Destination $staging
Copy-Item -LiteralPath (Join-Path $repository 'README.md') -Destination $staging
Copy-Item -LiteralPath (Join-Path $repository 'packaging\THIRD-PARTY-NOTICES.txt') -Destination $staging
Copy-Item -LiteralPath (Join-Path $repository 'packaging\DEPENDENCY-LICENSES.txt') -Destination $staging

$archive = "$staging.zip"
if (Test-Path -LiteralPath $archive) {
    Remove-Item -LiteralPath $archive -Force
}
Compress-Archive -Path (Join-Path $staging '*') -DestinationPath $archive -CompressionLevel Optimal
$hash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
Set-Content -LiteralPath "$archive.sha256" -Value "$hash  $([System.IO.Path]::GetFileName($archive))" -Encoding utf8NoBOM
Write-Output $archive
Write-Output "$archive.sha256"
