# Package the Windows release as MSIX for the Microsoft Store.
#
# Tauri's bundler emits .exe (NSIS) and .msi installers natively, but not
# MSIX — for that we stage the built binary + sidecars + manifest and
# call `makeappx.exe` from the installed Windows SDK. This script does
# the staging and packaging.
#
# Inputs:
#   - $env:TAG          — release tag (e.g. v1.2.3)
#   - $RustTarget arg   — Rust target triple (e.g. x86_64-pc-windows-msvc)
#
# Output:
#   - apps/desktop/src-tauri/target/release/bundle/msix/recast_<version>_x64.msix
#
# Inherent Windows-only because of `makeappx.exe`; PowerShell stays the
# native tool here (no point shoehorning into bash). The previous
# inline-in-YAML form was ~95 lines and hard to read; lifted into this
# file so the manifest XML, version coercion, and SDK probing can each
# be read top to bottom.

param(
    [Parameter(Mandatory = $true)]
    [string]$RustTarget
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = (Resolve-Path "$ScriptDir/../..").Path
Set-Location $RepoRoot

if (-not $env:TAG) {
    throw "TAG environment variable is required"
}
$version = $env:TAG.TrimStart('v')

# MSIX requires a 4-part numeric version (a.b.c.d). Pad with zeros and
# strip any pre-release suffix so packaging never trips on tags like
# v1.2.3-beta.
$core = ($version -split '[-+]')[0]
$parts = @($core -split '\.') | Where-Object { $_ -match '^\d+$' }
while ($parts.Count -lt 4) { $parts += '0' }
$msixVersion = ($parts[0..3] -join '.')

$releaseDir = "apps/desktop/src-tauri/target/release"
if (-not (Test-Path "$releaseDir/recast.exe")) {
    # Some Tauri targets nest the binary under the triple-specific path.
    $releaseDir = "apps/desktop/src-tauri/target/$RustTarget/release"
}
if (-not (Test-Path "$releaseDir/recast.exe")) {
    throw "recast.exe not found under target/release"
}

$staging = "msix-staging"
Remove-Item -Recurse -Force $staging -ErrorAction Ignore
New-Item -ItemType Directory -Force -Path "$staging/Assets" | Out-Null

Copy-Item "$releaseDir/recast.exe" "$staging/recast.exe"
Get-ChildItem $releaseDir -Filter "ffmpeg-*.exe" -ErrorAction Ignore |
    Copy-Item -Destination $staging
Get-ChildItem $releaseDir -Filter "ffprobe-*.exe" -ErrorAction Ignore |
    Copy-Item -Destination $staging
Get-ChildItem $releaseDir -Filter "*.dll" -ErrorAction Ignore |
    Copy-Item -Destination $staging
if (Test-Path "$releaseDir/resources") {
    Copy-Item -Recurse "$releaseDir/resources" "$staging/resources"
}

$icons = "apps/desktop/src-tauri/icons"
Copy-Item "$icons/Square150x150Logo.png" "$staging/Assets/Square150x150Logo.png"
Copy-Item "$icons/Square44x44Logo.png"   "$staging/Assets/Square44x44Logo.png"
Copy-Item "$icons/StoreLogo.png"         "$staging/Assets/StoreLogo.png"

$manifest = @"
<?xml version="1.0" encoding="utf-8"?>
<Package
  xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
  xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
  xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
  IgnorableNamespaces="uap rescap">
  <Identity Name="com.nexonauts.recast" Publisher="CN=Nexonauts" Version="$msixVersion" ProcessorArchitecture="x64" />
  <Properties>
    <DisplayName>Recast</DisplayName>
    <PublisherDisplayName>Nexonauts</PublisherDisplayName>
    <Logo>Assets\StoreLogo.png</Logo>
  </Properties>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.17763.0" MaxVersionTested="10.0.22621.0" />
  </Dependencies>
  <Resources>
    <Resource Language="en-us" />
  </Resources>
  <Applications>
    <Application Id="Recast" Executable="recast.exe" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements
        DisplayName="Recast"
        Description="Recast offline-first desktop recorder and editor"
        BackgroundColor="transparent"
        Square150x150Logo="Assets\Square150x150Logo.png"
        Square44x44Logo="Assets\Square44x44Logo.png" />
    </Application>
  </Applications>
  <Capabilities>
    <rescap:Capability Name="runFullTrust" />
  </Capabilities>
</Package>
"@
Set-Content -Path "$staging/AppxManifest.xml" -Value $manifest -Encoding UTF8

$makeappx = Get-ChildItem "C:/Program Files (x86)/Windows Kits/10/bin" -Recurse -Filter "makeappx.exe" -ErrorAction Ignore |
    Where-Object { $_.FullName -match '\\x64\\' } |
    Sort-Object FullName -Descending |
    Select-Object -First 1
if (-not $makeappx) { throw "makeappx.exe not found in installed Windows SDKs" }

$msixDir = "$releaseDir/bundle/msix"
New-Item -ItemType Directory -Force -Path $msixDir | Out-Null
$msixPath = "$msixDir/recast_${version}_x64.msix"
& $makeappx.FullName pack /d $staging /p $msixPath /o
if ($LASTEXITCODE -ne 0) { throw "makeappx pack failed with exit code $LASTEXITCODE" }
Write-Output "Created $msixPath"
