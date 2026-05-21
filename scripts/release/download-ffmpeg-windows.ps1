# Download FFmpeg + FFprobe for Windows x64 and place them as Tauri sidecars.
#
# gyan.dev's "release essentials" archive is the canonical Windows
# FFmpeg distribution; it bundles the encoders we need (libx264, aac,
# libvpx-vp9, libopus) and ships as a flat zip we can unpack directly.
#
# Inputs:
#   - $RustTarget arg — Rust target triple (e.g. x86_64-pc-windows-msvc)
#   - $Dest       arg — destination directory for sidecars

param(
    [Parameter(Mandatory = $true)]
    [string]$RustTarget,
    [Parameter(Mandatory = $true)]
    [string]$Dest
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = (Resolve-Path "$ScriptDir/../..").Path
Set-Location $RepoRoot

New-Item -ItemType Directory -Force -Path $Dest | Out-Null
$url = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"
Invoke-WebRequest -Uri $url -OutFile ffmpeg.zip -UseBasicParsing
Expand-Archive -Path ffmpeg.zip -DestinationPath ffmpeg-extract -Force
$bin = Get-ChildItem -Path ffmpeg-extract -Recurse -Filter "ffmpeg.exe" | Select-Object -First 1
$probe = Get-ChildItem -Path ffmpeg-extract -Recurse -Filter "ffprobe.exe" | Select-Object -First 1
if (-not $bin -or -not $probe) { throw "ffmpeg/ffprobe not found in archive" }
Copy-Item $bin.FullName "$Dest/ffmpeg-$RustTarget.exe" -Force
Copy-Item $probe.FullName "$Dest/ffprobe-$RustTarget.exe" -Force
Remove-Item -Recurse -Force ffmpeg.zip, ffmpeg-extract
