<#
.SYNOPSIS
    One-shot tool that downloads each built-in GGUF caption model and prints
    its SHA-256 alongside the `ggml_model(…)` registry call to paste in.

.DESCRIPTION
    Run once per release (or whenever the registry entries change), copy
    the printed lines into `apps/desktop/src-tauri/src/transcription/models.rs`,
    commit. The `download_file` / `is_installed` machinery already rejects
    mismatched downloads — pinning is the difference between a quiet
    re-download (current behavior) and a hard refusal (after this lands
    for every entry).

    Each model is downloaded into `%TEMP%/recast-pin-hashes/`. We're hashing
    real files because the registry's expected byte count can drift if the
    upstream repo gets re-uploaded; pinning by hash closes that hole.

.NOTES
    ⚠  This is a DEVELOPER TOOL, never shipped. It's `tools/dev/`, not
    `scripts/release/`. Lives there because the AGENTS.md convention has
    `scripts/` for things the CI/release pipelines invoke and `tools/`
    for ad-hoc developer aids.

    No CI invocation; no `actions/cache` step; no production rollout.
    Estimated runtime: ~3 minutes total (six large model files over HTTPS).
#>
[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$Entries = @(
    @{ id = 'parakeet-v3';     url = 'https://huggingface.co/handy-computer/parakeet-tdt-0.6b-v3-gguf/resolve/main/parakeet-tdt-0.6b-v3-Q8_0.gguf' },
    @{ id = 'parakeet-v2';     url = 'https://huggingface.co/handy-computer/parakeet-tdt-0.6b-v2-gguf/resolve/main/parakeet-tdt-0.6b-v2-Q8_0.gguf' },
    @{ id = 'whisper-base';    url = 'https://huggingface.co/handy-computer/whisper-base-gguf/resolve/main/whisper-base-Q5_K_M.gguf' },
    @{ id = 'whisper-small';   url = 'https://huggingface.co/handy-computer/whisper-small-gguf/resolve/main/whisper-small-Q5_K_M.gguf' }
)

$WorkDir = Join-Path $env:TEMP 'recast-pin-hashes'
$null = New-Item -ItemType Directory -Force -Path $WorkDir

Write-Host "Pinning $($Entries.Count) models into $WorkDir ..."
Write-Host "(Re-runs overwrite; downloaded files go to the OS temp dir.)"
Write-Host ""

$ProgressPreference = 'SilentlyContinue'  # silence Invoke-WebRequest's per-KiB ticker

foreach ($entry in $Entries) {
    $id  = $entry.id
    $url = $entry.url
    $dst = Join-Path $WorkDir "$id.gguf"

    Write-Host "── $id ──"
    Write-Host "  url : $url"

    if (-not (Test-Path $dst) -or (Get-Item $dst).Length -lt 10MB) {
        Write-Host "  downloading …"
        try {
            Invoke-WebRequest -Uri $url -OutFile $dst -UseBasicParsing -ErrorAction Stop
        } catch {
            Write-Warning "  failed: $_"
            Write-Host "  skipping $id"
            continue
        }
    } else {
        Write-Host "  using cached $dst"
    }

    $size = [math]::Round((Get-Item $dst).Length / 1MB, 1)
    $hash = (Get-FileHash -Path $dst -Algorithm SHA256).Hash.ToUpper()
    Write-Host "  size: $size MB"
    Write-Host "  sha256: $hash"
    Write-Host ""
    Write-Host "  → paste into apps/desktop/src-tauri/src/transcription/models.rs:"
    Write-Host "      ggml_model("
    Write-Host "          \"$id\","
    Write-Host "          …,"
    Write-Host "          Some(\"$hash\"),"
    Write-Host "      ),"
    Write-Host ""
}

Write-Host "Done. Review the hashes for stability (re-pin when the upstream"
Write-Host "HF file changes), copy the relevant Some(\"…\") lines into the"
Write-Host "registry, commit, and ship."
