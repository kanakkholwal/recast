use core::time::Duration;

use capturekit_core::{Result, Timestamp};

use crate::audio::{AudioBuffer, AudioCapturerBuilder, AudioHandle};
use crate::capturer::{CaptureHandle, CapturerBuilder, Flow, Frame};
use crate::platform::os;

/// How long a track waits on its source before looping to check for a stop.
const POLL: Duration = Duration::from_millis(250);

/// Names one stream inside a session, so a handler can tell them apart.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TrackId(pub String);

impl From<&str> for TrackId {
    fn from(name: &str) -> Self {
        Self(name.to_string())
    }
}

/// A frame, plus where it sits on the session's shared timeline.
pub struct SessionFrame<'a> {
    /// Which stream produced it.
    pub track: &'a TrackId,
    /// How far into the session the frame belongs.
    ///
    /// This, not [`Frame::pts`], is what aligns streams: every source on a given
    /// OS already stamps the same monotonic clock (QPC on Windows, the host time
    /// clock on macOS, `CLOCK_MONOTONIC` on Linux), so subtracting one origin
    /// puts them all on one timeline without any drift correction.
    pub elapsed: Duration,
    /// The frame itself.
    pub frame: Frame<'a>,
}

/// A buffer of samples, plus where it sits on the session's shared timeline.
pub struct SessionAudio<'a> {
    /// Which stream produced it.
    pub track: &'a TrackId,
    /// How far into the session the samples belong, on the same timeline as
    /// [`SessionFrame::elapsed`]. Lining audio up against video is subtracting
    /// one origin, nothing more.
    pub elapsed: Duration,
    /// The samples themselves.
    pub buffer: AudioBuffer<'a>,
}

/// Collects the streams a session will run before starting them together.
#[derive(Default)]
pub struct SessionBuilder {
    tracks: Vec<PendingTrack>,
}

enum PendingTrack {
    Video {
        id: TrackId,
        builder: CapturerBuilder,
        handler: Box<dyn FnMut(SessionFrame<'_>) -> Flow + Send>,
    },
    Audio {
        id: TrackId,
        builder: AudioCapturerBuilder,
        handler: Box<dyn FnMut(SessionAudio<'_>) -> Flow + Send>,
    },
}

/// One opened source, waiting for the origin to be read before it starts.
enum OpenTrack {
    Video(
        TrackId,
        crate::capturer::Capturer,
        Box<dyn FnMut(SessionFrame<'_>) -> Flow + Send>,
    ),
    Audio(
        TrackId,
        crate::audio::AudioCapturer,
        Box<dyn FnMut(SessionAudio<'_>) -> Flow + Send>,
    ),
}

impl OpenTrack {
    /// Start the source against a timeline origin every track shares.
    fn start(self, origin: Timestamp) -> TrackHandle {
        match self {
            Self::Video(id, capturer, mut handler) => {
                TrackHandle::Video(capturer.start(POLL, move |frame| {
                    let elapsed = frame.pts().saturating_since(origin);
                    handler(SessionFrame {
                        track: &id,
                        elapsed,
                        frame,
                    })
                }))
            }
            Self::Audio(id, capturer, mut handler) => {
                TrackHandle::Audio(capturer.start(POLL, move |buffer| {
                    let elapsed = buffer.pts().saturating_since(origin);
                    handler(SessionAudio {
                        track: &id,
                        elapsed,
                        buffer,
                    })
                }))
            }
        }
    }
}

/// A running track, whichever kind it is.
enum TrackHandle {
    Video(CaptureHandle),
    Audio(AudioHandle),
}

impl TrackHandle {
    fn stop(self) -> Result<()> {
        match self {
            Self::Video(handle) => handle.stop(),
            Self::Audio(handle) => handle.stop(),
        }
    }

    fn is_finished(&self) -> bool {
        match self {
            Self::Video(handle) => handle.is_finished(),
            Self::Audio(handle) => handle.is_finished(),
        }
    }
}

impl SessionBuilder {
    /// Add a video stream, with the handler that will run on its capture thread.
    ///
    /// Handlers run off the caller's thread, one thread per track, so a slow
    /// consumer on one stream cannot stall another.
    #[must_use]
    pub fn video<H>(mut self, id: impl Into<TrackId>, builder: CapturerBuilder, handler: H) -> Self
    where
        H: FnMut(SessionFrame<'_>) -> Flow + Send + 'static,
    {
        self.tracks.push(PendingTrack::Video {
            id: id.into(),
            builder,
            handler: Box::new(handler),
        });
        self
    }

    /// Add an audio stream, with the handler that will run on its capture thread.
    ///
    /// A microphone and a loopback capture are two tracks, not one: they are
    /// separate devices on separate clocks of their own, and mixing them is the
    /// consumer's decision, not this crate's.
    #[must_use]
    pub fn audio<H>(
        mut self,
        id: impl Into<TrackId>,
        builder: AudioCapturerBuilder,
        handler: H,
    ) -> Self
    where
        H: FnMut(SessionAudio<'_>) -> Flow + Send + 'static,
    {
        self.tracks.push(PendingTrack::Audio {
            id: id.into(),
            builder,
            handler: Box::new(handler),
        });
        self
    }

    /// Open every source and start them.
    ///
    /// Every source is opened before any is started, so a session that cannot be
    /// satisfied fails without having recorded a partial one. If one source
    /// fails, those already opened are dropped, which releases them.
    pub fn start(self) -> Result<Session> {
        let mut opened = Vec::with_capacity(self.tracks.len());
        for track in self.tracks {
            opened.push(match track {
                PendingTrack::Video {
                    id,
                    builder,
                    handler,
                } => OpenTrack::Video(id, builder.build()?, handler),
                PendingTrack::Audio {
                    id,
                    builder,
                    handler,
                } => OpenTrack::Audio(id, builder.build()?, handler),
            });
        }

        // Read once, after every source is open: a source that took a second to
        // negotiate must not shift the timeline the others are already on.
        let origin = os::now();

        let handles = opened
            .into_iter()
            .map(|track| track.start(origin))
            .collect();

        Ok(Session { origin, handles })
    }
}

/// Several capture streams running together on one timeline.
pub struct Session {
    origin: Timestamp,
    handles: Vec<TrackHandle>,
}

impl Session {
    /// Start building a session.
    #[must_use]
    pub fn builder() -> SessionBuilder {
        SessionBuilder::default()
    }

    /// The instant every track's `elapsed` is measured from.
    #[must_use]
    pub const fn origin(&self) -> Timestamp {
        self.origin
    }

    /// How many streams are running.
    #[must_use]
    pub fn track_count(&self) -> usize {
        self.handles.len()
    }

    /// Whether every stream has finished on its own.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.handles.iter().all(TrackHandle::is_finished)
    }

    /// Stop every stream and wait for all of them.
    ///
    /// Every track is stopped even if an earlier one failed, so one bad source
    /// cannot leave the others holding the desktop open. The first failure is
    /// what gets returned.
    pub fn stop(self) -> Result<()> {
        let mut first_error = None;
        for handle in self.handles {
            if let Err(err) = handle.stop() {
                first_error.get_or_insert(err);
            }
        }
        match first_error {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }
}
