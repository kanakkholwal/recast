<#
.SYNOPSIS
    End-to-end inference smoke test for a built recast binary.

.DESCRIPTION
    Drives the built recast binary through a real ggml inference pass
    and asserts no STATUS_ILLEGAL_INSTRUCTION (0xC000001D) on Windows,
    no SIGILL / SIGABRT on macOS / Linux, or other process-level abort
    fires. The Rust unit-test loop can't see those: a Rust panic is
    caught and reported, but a ggml `GGML_ASSERT(…) → GGML_ABORT(…) →
    abort()` kills the whole process; an in-process test dies with it,
    silently masking the signal. Spawning the binary as a child process
    + checking exit code + (Windows-only) post-mortem WER scan is the
    only reliable detection.

    Self-contained: one shell-out, one PowerShell script, all three
    GitHub-Actions runners (windows-latest / ubuntu-24.04 / macos-
    latest) supported via runtime `$IsWindows` / `$IsLinux` /
    `$IsMacOS` detection. The script:

      1. picks the right recast binary extension (`.exe` only on Windows),
      2. installs the bundled ffmpeg sidecar next to recast (assumed
         already downloaded by the upstream CI step that produced the
         smoke-test prerequisites; rejected as a hard fail if the sidecar
         is a 0-byte placeholder),
      3. downloads Whisper-base from Hugging Face into the repo's
         `models-cache/smoke-test/` directory (no cross-run cache — the
         user wants one step in the workflow, ~30 s download per CI run;
         for local dev the file persists in the repo),
      4. synthesizes a 1-second silent WAV (16 kHz mono PCM16) in-place,
      5. spawns the recast `transcribe` CLI verb, redirects stdout /
         stderr, and waits for the exit code (Linux maps SIGILL to
         exit 139, macOS maps SIGILL to exit 132 — both surface as
         `$exitCode != 0` and are caught by the verdict step),
      6. on Windows only, scans the WER `Application` event log for a
         fresh APPCRASH attributable to recast.exe.

    Used identically by `.github/workflows/ci-desktop.yml` and (when
    promoted) `.github/workflows/release-desktop.yml`. The CI workflow
    emits a single `run:` invocation; everything else lives inside.

.PARAMETER ExePath
    Absolute path to the recast binary. Default:
    $RepoRoot/apps/desktop/src-tauri/target/release/recast{,.exe}.

.PARAMETER RustTarget
    Target triple (`x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`,
    `aarch64-apple-darwin`, …). Used to find the bundled ffmpeg
    sidecar. Default: $env:RUST_TARGET, else platform-appropriate.

.PARAMETER RepoRoot
    Repo root. Default: $env:GITHUB_WORKSPACE, else the script's parent's
    parent (scripts/release → repo root).

.PARAMETER WorkDir
    Scratch dir for the synthetic WAV, model download, recast stdout/
    stderr. Default: $env:RUNNER_TEMP/recast-smoke-test (CI) or
    %TEMP%/recast-smoke-test (local).

.PARAMETER SkipWerScan
    Set to suppress the post-mortem WER scan. Linux / macOS never run
    it. Windows local devs with slow Event Viewer queries can flip this.

.PARAMETER DownloadTimeoutSeconds
    Max time to wait for the Whisper-base GGUF download. Default 300.

.NOTES
    The CLI verb (`recast transcribe …`) and its flag names are hard-
    coded below — there is no environment-variable override by design.
    The CI smoke test is the gate that catches CLI drift; an env override
    would mask it. When the verb changes, update the default in this
    script and the matching `// SMOKE_TEST_VERB:` comment in
    apps/desktop/src-tauri/src/cli.rs in the same commit.
#>
[CmdletBinding()]
param(
    [string] $ExePath,
    [string] $RustTarget,
    [string] $RepoRoot,
    [string] $WorkDir,
    [switch] $SkipWerScan,
    [int]    $DownloadTimeoutSeconds = 300
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

# ── platform detection ──────────────────────────────────────────────────────
#
# PowerShell 7 exposes $IsWindows / $IsLinux / $IsMacOS as READ-ONLY
# automatic variables — assigning to them throws `Cannot overwrite variable
# IsWindows because it is read-only or constant`. Read them, but never
# reassign. `$OnWindows` is our own local flag.

$OnWindows = ($IsWindows -eq $true) -or ($env:OS -eq 'Windows_NT')
$OnLinux   = ($IsLinux  -eq $true)
$OnMacOS   = ($IsMacOS  -eq $true)
$ExeExt    = if ($OnWindows) { '.exe' } else { '' }
Write-Host "Platform: $(if     ($OnWindows) {'Windows'} elseif ($OnLinux) {'Linux'} elseif ($OnMacOS) {'macOS'} else {'unknown'})"

# ── resolve defaults ────────────────────────────────────────────────────────

if (-not $RepoRoot) {
    $RepoRoot = $env:GITHUB_WORKSPACE
    if (-not $RepoRoot) {
        # scripts/release/smoke-test-transcription.ps1 → ../../ → repo root.
        $RepoRoot = (Resolve-Path "$PSScriptRoot/../..").Path
    }
}
Write-Host "RepoRoot: $RepoRoot"

if (-not $RustTarget) {
    $RustTarget = $env:RUST_TARGET
    if (-not $RustTarget) {
        if     ($OnWindows) { $RustTarget = 'x86_64-pc-windows-msvc' }
        elseif ($OnLinux)   { $RustTarget = 'x86_64-unknown-linux-gnu' }
        elseif ($OnMacOS)   { $RustTarget = 'aarch64-apple-darwin' }
        else                { $RustTarget = 'x86_64-unknown-linux-gnu' }
    }
}
Write-Host "RustTarget: $RustTarget"

if (-not $WorkDir) {
    $WorkDir = $env:RUNNER_TEMP
    if ($WorkDir) { $WorkDir = Join-Path $WorkDir 'recast-smoke-test' }
    else          { $WorkDir = Join-Path $env:TEMP   'recast-smoke-test' }
}
$null = New-Item -ItemType Directory -Force -Path $WorkDir
Write-Host "WorkDir: $WorkDir"

if (-not $ExePath) {
    $ExePath = Join-Path $RepoRoot "apps/desktop/src-tauri/target/release/recast$ExeExt"
}
if (-not (Test-Path $ExePath)) {
    throw "recast binary not found at $ExePath — the upstream `tauri build` (or `--no-bundle`) step did not produce one. To smoke-test a development build, set -ExePath to target/release/recast$ExeExt directly."
}
Write-Host "ExePath: $ExePath"

# Hard-coded CLI verb — the source of truth for the smoke-test contract.
# Mirror in apps/desktop/src-tauri/src/cli.rs (see the
# `// SMOKE_TEST_VERB:` marker comment around the transcribe verb).
$TranscribeVerb = 'transcribe --input "{wav}" --model "{gguf}" --out "{json}"'
Write-Host "TranscribeVerb: $TranscribeVerb"

# ── 1. install the bundled ffmpeg sidecar ───────────────────────────────────
#
# The CI / release workflows download a real ffmpeg into
# apps/desktop/src-tauri/binaries/ffmpeg-$RustTarget$sidecarExt before this
# script runs (that's a prerequisite of `tauri::generate_context!`, which
# validates externalBin sidecars at macro-expansion time). The script
# just installs that sidecar next to the recast binary so the
# `Command::new("ffmpeg")` inside audio::extract_pcm_f32 resolves.

$sidecarExt = if ($OnWindows) { '.exe' } else { '' }
$sidecarSrc = Join-Path $RepoRoot "apps/desktop/src-tauri/binaries/ffmpeg-$RustTarget$sidecarExt"
$binDir     = Split-Path -Parent $ExePath
$sidecarDst = Join-Path $binDir "ffmpeg$sidecarExt"

if (Test-Path $sidecarSrc) {
    # 0-byte sidecars are a CI misconfiguration: `tauri::generate_context!`
    # accepts an empty file at compile time, but the runtime spawn fails
    # with cryptic OS error 193 (`%1 is not a valid Win32 application`)
    # on Windows. Refuse the install + throw with a message that points
    # at the missing CI step rather than letting recast fail downstream.
    $sidecarSize = (Get-Item $sidecarSrc).Length
    if ($sidecarSize -eq 0) {
        throw "ffmpeg sidecar at $sidecarSrc is a 0-byte placeholder. The upstream 'Download FFmpeg sidecars' CI step should have written a real binary at this path; check that the cache + download-ffmpeg-{windows,macos,linux} jobs ran before this point."
    }
    if (-not (Test-Path $sidecarDst) -or
        (Get-Item $sidecarSrc).LastWriteTimeUtc -gt (Get-Item $sidecarDst).LastWriteTimeUtc) {
        Copy-Item -Force $sidecarSrc $sidecarDst
        Write-Host "✓ installed ffmpeg sidecar → $sidecarDst ($([math]::Round($sidecarSize / 1MB, 1)) MB)"
    }
} elseif (Test-Path $sidecarDst) {
    Write-Host "✓ ffmpeg sidecar already at → $sidecarDst ($([math]::Round((Get-Item $sidecarDst).Length / 1MB, 1)) MB)"
} else {
    throw "ffmpeg sidecar missing — install ffmpeg into apps/desktop/src-tauri/binaries/ffmpeg-$RustTarget$sidecarExt before running this script."
}

# ── 2. ensure the Whisper-base GGUF is present ──────────────────────────────
#
# Downloads once per CI run (~30 s for ~60 MB) into the repo's
# models-cache/smoke-test/. The CI workflow does NOT cache this
# directory across runs — the user's directive was "one step"; that
# 30 s overhead is the trade-off. For local dev the file persists in
# the repo across sessions and is reused automatically.
#
# HF repo + filename mirror the registry entry at
# apps/desktop/src-tauri/src/transcription/models.rs:218 (parakeet-v3 +
# whisper-base are also covered there — they share the smoke-test
# contract).

$ggufDir  = Join-Path $RepoRoot 'models-cache/smoke-test'
$null     = New-Item -ItemType Directory -Force -Path $ggufDir
$ggufPath = Join-Path $ggufDir 'whisper-base-Q5_K_M.gguf'

$needsDownload = $true
if (Test-Path $ggufPath) {
    $existingSize = (Get-Item $ggufPath).Length
    # Sanity floor: a real Whisper-base GGUF is ~60 MB. A file shorter than
    # this is truncated/corrupt — re-download. (Whisper-base at Q5_K_M is
    # about 77 MB on disk per its hugging-face metadata.)
    if ($existingSize -ge 50MB) {
        $needsDownload = $false
        Write-Host "✓ GGUF already cached: $ggufPath ($([math]::Round($existingSize / 1MB, 1)) MB)"
    }
}
if ($needsDownload) {
    Write-Host "Downloading smoke-test model to $ggufPath …"
    $url = 'https://huggingface.co/handy-computer/whisper-base-gguf/resolve/main/whisper-base-Q5_K_M.gguf'
    try {
        $ProgressPreference = 'SilentlyContinue'  # silence Invoke-WebRequest's per-KiB ticker
        $job = Start-ThreadJob -ArgumentList $url, $ggufPath -ScriptBlock {
            param($u, $o)
            Invoke-WebRequest -Uri $u -OutFile $o -UseBasicParsing -ErrorAction Stop
        }
        if (-not (Wait-Job $job -Timeout $DownloadTimeoutSeconds)) {
            Stop-Job $job -Force
            throw "Model download timed out after $DownloadTimeoutSeconds seconds — see $ggufDir."
        }
        Receive-Job $job -Wait -AutoRemoveJob
    } catch {
        throw "Failed to download smoke-test model from HF: $_`n`nIf this is a corporate firewall or CI proxy issue, pre-bake the file into the repo under models-cache/smoke-test/ instead."
    }
    Write-Host "✓ GGUF ready: $ggufPath ($([math]::Round((Get-Item $ggufPath).Length / 1MB, 1)) MB)"
}

# ── 3. synthesize a 1-second silent WAV (16 kHz mono PCM16) ─────────────────

$wavPath = Join-Path $WorkDir 'silence-1s.wav'

# WAV header (44 bytes) + 32 000 bytes of zero PCM. Inline so no fixture
# has to live in the repo, and no `dd.exe` / `printf | xxd` gymnastics per OS.
$header = [byte[]](
    0x52,0x49,0x46,0x46, 0x00,0x00,0x00,0x00, 0x57,0x41,0x56,0x45,
    0x66,0x6d,0x74,0x20, 0x10,0x00,0x00,0x00, 0x01,0x00, 0x01,0x00,
    0x80,0x3e,0x00,0x00, 0x00,0x7d,0x00,0x00, 0x02,0x00, 0x10,0x00,
    0x64,0x61,0x74,0x61, 0x00,0x00,0x00,0x00
)
$dataChunkSize = 32000  # 16 kHz * 1 s * 2 bytes/sample
$riffSize      = $header.Length + $dataChunkSize - 8
[BitConverter]::GetBytes([uint32]$riffSize).CopyTo($header, 4)
[BitConverter]::GetBytes([uint32]$dataChunkSize).CopyTo($header, 40)

$wavBytes = New-Object 'System.Collections.Generic.List[byte]'($header.Length + $dataChunkSize)
$wavBytes.AddRange($header)
for ($i = 0; $i -lt $dataChunkSize; $i++) { $wavBytes.Add(0) }
[System.IO.File]::WriteAllBytes($wavPath, $wavBytes.ToArray())
Write-Host "✓ wrote 1-second silent WAV: $wavPath ($($wavBytes.Count) bytes)"

# ── 4. run recast transcribe ────────────────────────────────────────────────
#
# Start-Process's `-RedirectStandardOutput` / `-RedirectStandardError`
# flags work in PowerShell 7 on Linux / macOS as well as Windows; the
# call operator with `&` plus redirections has cross-shell quirks on
# Unix, so we standardize on Start-Process. Exit code is the universal
# signal (Windows: 0 for clean, anything else including
# 0xC000001D-based WER triggers 1 via `run_and_exit`. Linux maps
# SIGILL → exit 139, macOS maps SIGILL → exit 132 — both fail the
# `$exitCode -ne 0` check below.).

$jsonPath = Join-Path $WorkDir 'smoke-out.json'
$verb = $TranscribeVerb -replace '\{wav\}',  "`"$wavPath`"" `
                        -replace '\{gguf\}', "`"$ggufPath`"" `
                        -replace '\{json\}', "`"$jsonPath`""

$stdoutLog = Join-Path $WorkDir 'stdout.log'
$stderrLog = Join-Path $WorkDir 'stderr.log'
Write-Host "→ `"$ExePath`" $verb"

$proc = Start-Process -FilePath $ExePath `
                       -ArgumentList $verb `
                       -PassThru -Wait `
                       -RedirectStandardOutput $stdoutLog `
                       -RedirectStandardError  $stderrLog
$exitCode = $proc.ExitCode
Write-Host "recast exited with code $exitCode"
if (Test-Path $stdoutLog) { Write-Host "stdout:"; Get-Content $stdoutLog | Select-Object -First 20 }
if (Test-Path $stderrLog) { Write-Host "stderr:"; Get-Content $stderrLog | Select-Object -First 20 }

# ── 5. WER scan (Windows only) ─────────────────────────────────────────────
#
# Linux / macOS: process exit code is the only universally portable
# signal; the verdict step covers it.

if (-not $SkipWerScan -and $OnWindows) {
    $since   = (Get-Date).AddSeconds(-180)
    $werHits = Get-WinEvent -FilterHashtable @{
                     LogName   = 'Application'
                     StartTime = $since
                 } -ErrorAction SilentlyContinue |
                 Where-Object {
                     ($_.ProviderName -in @('Application Error', '.NET Runtime')) -and
                     ($_.Message -match 'recast\.exe')
                 }
    if ($werHits) {
        $sample = $werHits | Select-Object -First 1
        $werDetails = $sample.ToXml()
        throw @"
smoke test detected APPCRASH for recast.exe within the last 180 s:

  Provider: $($sample.ProviderName)
  Time:     $($sample.TimeCreated)
  Event:    $($sample.Id) / $($sample.Qualifiers)

Get full event via:
  Get-WinEvent -LogName Application | ? Message -match 'recast.exe' | Select -First 1 | Format-List

$werDetails
"@
    }
}

# ── 6. verdict ──────────────────────────────────────────────────────────────

if ($exitCode -ne 0) {
    throw "recast transcribe exited non-zero ($exitCode). See $stdoutLog and $stderrLog."
}

if (-not (Test-Path $jsonPath)) {
    Write-Warning "No JSON at $jsonPath — transcription produced empty output (likely a real silence run). Process exited cleanly; carrying on."
} else {
    Write-Host "✓ transcription output: $jsonPath"
}

Write-Host "`n✓ smoke test passed (exit=0, no APPCRASH)"
exit 0
