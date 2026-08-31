//! An FFmpeg-backed codec pair for platforms with no in-process encoder.
//!
//! The compositing is still ours: FFmpeg only decodes the recording and encodes
//! what the engine drew, the same division the browser renderer already ships.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};

use recast_compositor::{PlaneData, PlaneLayout, SourceColor, SourcePlanes};

use crate::frames::PictureSource;

/// Stderr kept for diagnostics. The fatal line is always last; the banner is noise.
const STDERR_TAIL: usize = 8192;

#[derive(Debug, thiserror::Error)]
pub enum FfmpegError {
    #[error("starting {program}: {error}")]
    Spawn {
        program: PathBuf,
        #[source]
        error: std::io::Error,
    },
    #[error("reading a decoded frame: {0}")]
    Read(#[source] std::io::Error),
    #[error("writing a rendered frame: {0}")]
    Write(#[source] std::io::Error),
    #[error("the child's {stream} was not available")]
    NoPipe { stream: &'static str },
    #[error("ffmpeg exited with {status}: {tail}")]
    Exited { status: String, tail: String },
    #[error("a frame is {got} bytes, not the {want} this geometry needs")]
    FrameSize { got: usize, want: usize },
    #[error("the source has no frame rate to index frames by")]
    NoFrameRate,
}

/// A child's stderr, drained on a side thread into a bounded tail.
///
/// Draining is load-bearing, not diagnostic: a long-lived FFmpeg whose stderr
/// fills its pipe buffer blocks, and a sink writing to its stdin then deadlocks.
struct StderrTail(Arc<Mutex<String>>);

impl StderrTail {
    fn drain(child: &mut Child) -> Self {
        let tail = Arc::new(Mutex::new(String::new()));
        if let Some(mut stderr) = child.stderr.take() {
            let sink = Arc::clone(&tail);
            std::thread::spawn(move || {
                let mut buffer = [0u8; 4096];
                while let Ok(read) = stderr.read(&mut buffer) {
                    if read == 0 {
                        break;
                    }
                    let Ok(mut held) = sink.lock() else { break };
                    held.push_str(&String::from_utf8_lossy(&buffer[..read]));
                    if held.len() > STDERR_TAIL {
                        let cut = held.len() - STDERR_TAIL;
                        held.drain(..cut);
                    }
                }
            });
        }
        Self(tail)
    }

    fn get(&self) -> String {
        self.0
            .lock()
            .map(|t| t.trim().to_string())
            .unwrap_or_default()
    }
}

fn spawn(
    program: &Path,
    args: &[String],
    stdin: Stdio,
    stdout: Stdio,
) -> Result<Child, FfmpegError> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(stdin)
        .stdout(stdout)
        .stderr(Stdio::piped());
    silence(&mut command);
    command.spawn().map_err(|error| FfmpegError::Spawn {
        program: program.to_path_buf(),
        error,
    })
}

/// No console window on Windows, where a spawn otherwise flashes one and steals focus.
fn silence(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = command;
}

/// What the caller already probed about the recording, so this module never
/// shells out to ffprobe of its own.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceInfo {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
}

impl SourceInfo {
    fn frame_bytes(&self) -> usize {
        PlaneLayout::Nv12.packed_len(self.width, self.height)
    }

    /// The frame covering `t`, on the constant-rate axis the decode is forced onto.
    fn index_at(&self, t: f64) -> u64 {
        (t.max(0.0) * self.fps).floor() as u64
    }
}

/// A recording decoded through FFmpeg into NV12, one frame at a time.
///
/// Decoding is forced to a constant rate so a frame index is a time: a raw pipe
/// carries no timestamps, and the alternative is trusting a variable-rate
/// source to report its own.
pub struct FfmpegPictures {
    program: PathBuf,
    input: PathBuf,
    info: SourceInfo,
    color: SourceColor,
    child: Option<Child>,
    stderr: Option<StderrTail>,
    frame: Vec<u8>,
    /// Index of the frame held in `frame`, or `None` before the first read.
    at: Option<u64>,
}

impl FfmpegPictures {
    /// Opens `input` for decoding. Nothing spawns until the first frame is asked
    /// for, so the geometry is validated where the caller can still report it.
    pub fn open(
        program: &Path,
        input: &Path,
        info: SourceInfo,
        color: SourceColor,
    ) -> Result<Self, FfmpegError> {
        if !(info.fps.is_finite() && info.fps > 0.0) {
            return Err(FfmpegError::NoFrameRate);
        }
        Ok(Self {
            program: program.to_path_buf(),
            input: input.to_path_buf(),
            info,
            color,
            child: None,
            stderr: None,
            frame: vec![0u8; info.frame_bytes()],
            at: None,
        })
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.info.width
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.info.height
    }

    /// Restarts the decode at `index`, which is how this backend seeks: a raw
    /// pipe cannot rewind.
    fn restart(&mut self, index: u64) -> Result<(), FfmpegError> {
        self.stop();
        let seconds = index as f64 / self.info.fps;
        let args = vec![
            "-hide_banner".into(),
            "-loglevel".into(),
            "error".into(),
            // Before -i so it seeks by keyframe rather than decoding the whole head.
            "-ss".into(),
            format!("{seconds:.6}"),
            "-i".into(),
            self.input.to_string_lossy().into_owned(),
            "-vf".into(),
            format!("fps={}", self.info.fps),
            "-pix_fmt".into(),
            "nv12".into(),
            "-f".into(),
            "rawvideo".into(),
            "-".into(),
        ];
        let mut child = spawn(&self.program, &args, Stdio::null(), Stdio::piped())?;
        self.stderr = Some(StderrTail::drain(&mut child));
        self.child = Some(child);
        // The next read produces `index`, so the held index is one before it.
        self.at = index.checked_sub(1);
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.stderr = None;
    }

    /// Reads one whole frame, or `false` at end of stream.
    fn read_frame(&mut self) -> Result<bool, FfmpegError> {
        let Some(child) = self.child.as_mut() else {
            return Ok(false);
        };
        let stdout = child
            .stdout
            .as_mut()
            .ok_or(FfmpegError::NoPipe { stream: "stdout" })?;
        let mut filled = 0usize;
        while filled < self.frame.len() {
            match stdout.read(&mut self.frame[filled..]) {
                Ok(0) => break,
                Ok(read) => filled += read,
                Err(error) => return Err(FfmpegError::Read(error)),
            }
        }
        if filled == 0 {
            return Ok(false);
        }
        // A short read is a torn picture, which would render rather than fail.
        if filled < self.frame.len() {
            return Err(FfmpegError::FrameSize {
                got: filled,
                want: self.frame.len(),
            });
        }
        self.at = Some(self.at.map_or(0, |a| a + 1));
        Ok(true)
    }
}

impl Drop for FfmpegPictures {
    fn drop(&mut self) {
        self.stop();
    }
}

impl PictureSource for FfmpegPictures {
    type Error = FfmpegError;

    fn picture_at(&mut self, source_time: f64) -> Result<Option<SourcePlanes<'_>>, Self::Error> {
        let want = self.info.index_at(source_time);
        let behind = self.at.is_some_and(|at| want < at);
        if self.child.is_none() || behind {
            self.restart(want)?;
        }
        while self.at.is_none_or(|at| at < want) {
            if !self.read_frame()? {
                break;
            }
        }
        // Past the end the last decoded frame stands, matching the native reader.
        if self.at.is_none() {
            return Ok(None);
        }
        Ok(Some(SourcePlanes {
            width: self.info.width,
            height: self.info.height,
            layout: PlaneLayout::Nv12,
            color: self.color,
            data: PlaneData::Packed(&self.frame),
        }))
    }
}

/// An H.264 encoder fed the engine's rendered RGBA on stdin.
///
/// Video only: the audio track is muxed by the pass that owns the music clips,
/// which is also the pass that turns this into a GIF or a WebM.
pub struct FfmpegSink {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stderr: Option<StderrTail>,
    frame_bytes: usize,
}

impl FfmpegSink {
    /// Opens an encoder writing `output` at `width` by `height` and `fps`.
    pub fn new(
        program: &Path,
        output: &Path,
        width: u32,
        height: u32,
        fps: (u32, u32),
        bitrate: u32,
    ) -> Result<Self, FfmpegError> {
        let args = vec![
            "-hide_banner".into(),
            "-loglevel".into(),
            "error".into(),
            "-y".into(),
            "-f".into(),
            "rawvideo".into(),
            "-pix_fmt".into(),
            "rgba".into(),
            "-s".into(),
            format!("{width}x{height}"),
            "-r".into(),
            format!("{}/{}", fps.0, fps.1.max(1)),
            "-i".into(),
            "-".into(),
            "-an".into(),
            "-c:v".into(),
            "libx264".into(),
            "-preset".into(),
            "veryfast".into(),
            "-b:v".into(),
            bitrate.to_string(),
            // The engine works in RGBA; everything downstream expects 4:2:0.
            "-pix_fmt".into(),
            "yuv420p".into(),
            "-movflags".into(),
            "+faststart".into(),
            output.to_string_lossy().into_owned(),
        ];
        let mut child = spawn(program, &args, Stdio::piped(), Stdio::null())?;
        let stderr = StderrTail::drain(&mut child);
        let stdin = child
            .stdin
            .take()
            .ok_or(FfmpegError::NoPipe { stream: "stdin" })?;
        Ok(Self {
            child: Some(child),
            stdin: Some(stdin),
            stderr: Some(stderr),
            frame_bytes: width as usize * height as usize * 4,
        })
    }

    /// Writes one rendered frame. This backend has no random access and takes
    /// frames in the order the loop produces them.
    pub fn push(&mut self, rgba: &[u8]) -> Result<(), FfmpegError> {
        if rgba.len() < self.frame_bytes {
            return Err(FfmpegError::FrameSize {
                got: rgba.len(),
                want: self.frame_bytes,
            });
        }
        let stdin = self
            .stdin
            .as_mut()
            .ok_or(FfmpegError::NoPipe { stream: "stdin" })?;
        stdin
            .write_all(&rgba[..self.frame_bytes])
            .map_err(FfmpegError::Write)
    }

    /// Closes the input and waits for the file to be written.
    pub fn finish(mut self) -> Result<(), FfmpegError> {
        // Dropped before the wait: ffmpeg exits on end of input, and holding this deadlocks.
        self.stdin = None;
        let Some(mut child) = self.child.take() else {
            return Ok(());
        };
        let status = child.wait().map_err(FfmpegError::Read)?;
        if status.success() {
            return Ok(());
        }
        Err(FfmpegError::Exited {
            status: status.to_string(),
            tail: self
                .stderr
                .as_ref()
                .map(StderrTail::get)
                .unwrap_or_default(),
        })
    }
}

impl Drop for FfmpegSink {
    fn drop(&mut self) {
        self.stdin = None;
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
