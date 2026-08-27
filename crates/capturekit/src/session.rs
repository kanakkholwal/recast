use core::time::Duration;

use capturekit_core::{Result, Timestamp};

use crate::capturer::{CaptureHandle, CapturerBuilder, Flow, Frame};
use crate::platform::os;

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

/// Collects the streams a session will run before starting them together.
#[derive(Default)]
pub struct SessionBuilder {
    tracks: Vec<PendingTrack>,
}

struct PendingTrack {
    id: TrackId,
    builder: CapturerBuilder,
    handler: Box<dyn FnMut(SessionFrame<'_>) -> Flow + Send>,
    timeout: Duration,
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
        self.tracks.push(PendingTrack {
            id: id.into(),
            builder,
            handler: Box::new(handler),
            timeout: Duration::from_millis(250),
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
            let capturer = track.builder.build()?;
            opened.push((track.id, capturer, track.handler, track.timeout));
        }

        // Read once, after every source is open: a source that took a second to
        // negotiate must not shift the timeline the others are already on.
        let origin = os::now();

        let handles = opened
            .into_iter()
            .map(|(id, capturer, mut handler, timeout)| {
                capturer.start(timeout, move |frame| {
                    let elapsed = frame.pts().saturating_since(origin);
                    handler(SessionFrame {
                        track: &id,
                        elapsed,
                        frame,
                    })
                })
            })
            .collect();

        Ok(Session { origin, handles })
    }
}

/// Several capture streams running together on one timeline.
pub struct Session {
    origin: Timestamp,
    handles: Vec<CaptureHandle>,
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
        self.handles.iter().all(CaptureHandle::is_finished)
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
