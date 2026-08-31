/// True for an FFmpeg `-progress` metric line, matching the keys `print_report()` writes.
/// Filtered out of the error ring, or a successful export's progress stream pushes the real error off the tail.
pub(crate) fn is_ffmpeg_progress_key_line(line: &str) -> bool {
    const KEYS: &[&str] = &[
        "frame=",
        "fps=",
        "bitrate=",
        "total_size=",
        "out_time_ms=",
        "out_time=",
        "dup_frames=",
        "drop_frames=",
        "speed=",
        "progress=",
    ];
    let trimmed = line.trim_start();
    if trimmed.starts_with("stream_") {
        // e.g. `stream_0_0_q=28.0`
        return true;
    }
    KEYS.iter().any(|k| trimmed.starts_with(k))
}

pub(crate) fn parse_ffmpeg_progress_seconds(line: &str) -> Option<f64> {
    if let Some(value) = line
        .strip_prefix("out_time_us=")
        .or_else(|| line.strip_prefix("out_time_ms="))
    {
        return value
            .trim()
            .parse::<f64>()
            .ok()
            .map(|raw| raw / 1_000_000.0);
    }

    let value = line.strip_prefix("out_time=")?.trim();
    let mut parts = value.split(':');
    let hours = parts.next()?.parse::<f64>().ok()?;
    let minutes = parts.next()?.parse::<f64>().ok()?;
    let seconds = parts.next()?.parse::<f64>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

/// Maps a stage's raw 0..100 progress onto the export's overall bar. A single
/// pass owns the whole bar (`{offset: 0, scale: 1}`); a 2-pass GIF splits it
/// (pre-pass `{0, 0.4}`, main pass `{40, 0.6}`).
#[derive(Clone, Copy)]
pub(crate) struct ProgressBand {
    pub(crate) offset: f64,
    pub(crate) scale: f64,
}

impl ProgressBand {
    pub(crate) fn at(self, raw_pct: f64) -> f64 {
        self.offset + self.scale * raw_pct
    }
}
