# FFmpeg Sidecars

Place bundled FFmpeg binaries in this directory using Tauri's target-triple
sidecar naming convention:

- `ffmpeg-x86_64-pc-windows-msvc.exe`
- `ffprobe-x86_64-pc-windows-msvc.exe`

For other targets, use the target triple that Tauri builds for, for example:

- `ffmpeg-aarch64-apple-darwin`
- `ffprobe-aarch64-apple-darwin`
- `ffmpeg-x86_64-unknown-linux-gnu`
- `ffprobe-x86_64-unknown-linux-gnu`

The app also accepts plain `ffmpeg(.exe)` and `ffprobe(.exe)` next to the app
or under a `bin/` directory for local development, but release builds should use
the Tauri sidecar names above.
