<#
.SYNOPSIS
    End-to-end inference smoke test for a built recast binary.

.DESCRIPTION
    Drives the built recast.exe through a real ggml inference pass and
    asserts no STATUS_ILLEGAL_INSTRUCTION (0xC000001D) or other native
    abort fires. The Rust unit-test loop can't see those: a Rust panic is
    caught and reported, but a ggml `GGML_ASSERT(…) → GGML_ABORT(…) →
    abort()` kills the whole process; an in-process test dies with it,
    silently masking the signal. Spawning the binary as a child process
    + checking exit code + post-mortem WER scan is the only reliable
    detection.

    Reused unchanged from both `.github/workflows/ci-desktop.yml` and
    `.github/workflows/release-desktop.yml` — same flags, same fixture
    (a synthesized 1-second silent WAV), same model download (Whisper-base,
    the smallest production GGUF in apps/desktop/src-tauri/src/transcription/
    models.rs:218). This is a CI/release step, not an integration test in
    the recast crate — running it through the actual packaged binary is
    the point.

    The portable-x64 ggml build flag (`TRANSCRIBE_CMAKE_ARGS`) belongs on
    the COMPILATION step (tauri build), not here. By the time the smoke
    test runs the binary is already built; re-setting the cmake flag would
    be a no-op. Wire it into the upstream job's environment alongside the
    CMake build invocation.

.PARAMETER ExePath
    Absolute path to recast.exe. Default: $RepoRoot/apps/desktop/src-tauri/
    target/release/recast.exe.

.PARAMETER RustTarget
    Target triple (`x86_64-pc-windows-msvc` etc.). Used to find the bundled
    ffmpeg sidecar. Default: $env:RUST_TARGET or x86_64-pc-windows-msvc.

.PARAMETER RepoRoot
    Repo root. Default: $env:GITHUB_WORKSPACE or the script's parent's
    parent (scripts/release → repo root).

.PARAMETER WorkDir
    Scratch dir for the synthetic WAV, model download, recast stdout/stderr.
    Default: $env:RUNNER_TEMP/recast-smoke-test (CI) or %TEMP%/recast-smoke-
    test (local).

.PARAMETER SkipWerScan
    Set to suppress the post-mortem WER scan (used by local devs where
    Event Viewer queries are slow and ungated). CI leaves this unset.

.PARAMETER DownloadTimeoutSeconds
    Max time to wait for the smoke-test model download. Default 300.

.NOTES
    The CLI verb used here (`recast transcribe …`) is hard-coded. There is
    no environment-variable override by design — the smoke test must fail
    loudly the moment the verb in apps/desktop/src-tauri/src/cli.rs drifts
    from the shape below. The CI smoke test is the gate that catches the
    drift; an env override would mask it. When the verb changes, update
    this default and the matching `// SMOKE_TEST_VERB:` comment in cli.rs.
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

# ── resolve defaults ────────────────────────────────────────────────────────

if (-not $RepoRoot) {
    $RepoRoot = $env:GITHUB_WORKSPACE
    if (-not $RepoRoot) {
        # scripts/release/smoke-test-transcription.ps1 → ../../ → repo root.
        $RepoRoot = (Resolve-Path "$PSScriptRoot\..\..").Path
    }
}
Write-Host "RepoRoot: $RepoRoot"

if (-not $ExePath) {
    $ExePath = Join-Path $RepoRoot "apps\desktop\src-tauri\target\release\recast.exe"
}
if (-not (Test-Path $ExePath)) {
    throw "recast binary not found at $ExePath — the upstream `tauri build` (or `--no-bundle`) step did not produce one."
}
Write-Host "ExePath: $ExePath"

if (-not $RustTarget) {
    $RustTarget = $env:RUST_TARGET
    if (-not $RustTarget) { $RustTarget = "x86_64-pc-windows-msvc" }
}
Write-Host "RustTarget: $RustTarget"

if (-not $WorkDir) {
    $WorkDir = $env:RUNNER_TEMP
    if ($WorkDir) { $WorkDir = Join-Path $WorkDir "recast-smoke-test" }
    else          { $WorkDir = Join-Path $env:TEMP   "recast-smoke-test" }
}
$null = New-Item -ItemType Directory -Force -Path $WorkDir
Write-Host "WorkDir: $WorkDir"

# Hard-coded CLI verb — the source of truth for the smoke-test contract.
# Mirror this in apps/desktop/src-tauri/src/cli.rs (see the
# `// SMOKE_TEST_VERB:` marker comment around the transcribe verb).
$TranscribeVerb = 'transcribe --input "{wav}" --model "{gguf}" --out "{json}"'
Write-Host "TranscribeVerb: $TranscribeVerb"

# ── 1. synthesize a 1-second silent WAV (16 kHz mono PCM16) ─────────────────

$wavPath = Join-Path $WorkDir "silence-1s.wav"

# WAV header (44 bytes) + 32 000 bytes of zero PCM. Inline so no fixture has
# to live in the repo, and no `dd.exe` / `printf | xxd` gymnastics per OS.
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

# ── 2. ensure Whisper-base GGUF is present (HF repo + filename mirror the
#      registry at apps/desktop/src-tauri/src/transcription/models.rs:218) ──

$ggufDir = Join-Path $RepoRoot "models-cache\smoke-test"
$null = New-Item -ItemType Directory -Force -Path $ggufDir
$ggufPath = Join-Path $ggufDir "whisper-base-Q5_K_M.gguf"

if (-not (Test-Path $ggufPath)) {
    Write-Host "Downloading smoke-test model to $ggufPath …"
    $url = "https://huggingface.co/handy-computer/whisper-base-gguf/resolve/main/whisper-base-Q5_K_M.gguf"
    try {
        $ProgressPreference = 'SilentlyContinue'   # silence Invoke-WebRequest's per-KiB ticker
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
}
$ggufSizeMB = [math]::Round((Get-Item $ggufPath).Length / 1MB, 1)
Write-Host "✓ GGUF ready: $ggufPath ($ggufSizeMB MB)"

# ── 3. install the bundled ffmpeg sidecar alongside the binary ────────────

$sidecarSrc = Join-Path $RepoRoot "apps\desktop\src-tauri\binaries\ffmpeg-$RustTarget.exe"
$binDir     = Split-Path -Parent $ExePath
$sidecarDst = Join-Path $binDir "ffmpeg.exe"
if (Test-Path $sidecarSrc) {
    if (-not (Test-Path $sidecarDst) -or
        (Get-Item $sidecarSrc).LastWriteTimeUtc -gt (Get-Item $sidecarDst).LastWriteTimeUtc) {
        Copy-Item -Force $sidecarSrc $sidecarDst
        Write-Host "✓ installed ffmpeg sidecar → $sidecarDst"
    }
} elseif (-not (Test-Path $sidecarDst)) {
    Write-Warning "ffmpeg sidecar not bundled — recast will fail to extract audio and the JSON output will be empty. SIGILL detection still works."
}

# ── 4. run recast transcribe ────────────────────────────────────────────────

$jsonPath = Join-Path $WorkDir "smoke-out.json"
$verb = $TranscribeVerb -replace '\{wav\}',  "`"$wavPath`"" `
                        -replace '\{gguf\}', "`"$ggufPath`"" `
                        -replace '\{json\}', "`"$jsonPath`""

$stdoutLog = Join-Path $WorkDir "stdout.log"
$stderrLog = Join-Path $WorkDir "stderr.log"
Write-Host "→ & `"$ExePath`" $verb"

$proc = Start-Process -FilePath $ExePath `
                       -ArgumentList $verb `
                       -NoNewWindow -PassThru -Wait `
                       -RedirectStandardOutput $stdoutLog `
                       -RedirectStandardError  $stderrLog
$exitCode = $proc.ExitCode
Write-Host "recast exited with code $exitCode"
if (Test-Path $stdoutLog) { Write-Host "stdout:"; Get-Content $stdoutLog | Select-Object -First 20 }
if (Test-Path $stderrLog) { Write-Host "stderr:"; Get-Content $stderrLog | Select-Object -First 20 }

# ── 5. WER scan (skip when running locally) ─────────────────────────────────

if (-not $SkipWerScan) {
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
  Get-WinEvent -LogName Application | ? Message -match 'recast\.exe' | Select -First 1 | Format-List

$werDetails
"@
    }
}

# ── 6. verdict ──────────────────────────────────────────────────────────────

if ($exitCode -ne 0) {
    throw "recast transcribe exited non-zero ($exitCode). See $stdoutLog and $stderrLog."
}

if (-not (Test-Path $jsonPath)) {
    Write-Warning "No JSON at $jsonPath — ffmpeg sidecar likely missing. Process exited cleanly; carrying on."
} else {
    Write-Host "✓ transcription output: $jsonPath"
}

Write-Host "`n✓ smoke test passed (exit=0, no APPCRASH)"
exit 0
