use core::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use capturekit_core::{
    AudioDesc, AudioDevice, AudioDeviceId, AudioDirection, AudioFormat, AudioTimeline,
    CaptureError, Result, SampleFormat, Timestamp,
};
use pipewire::spa::param::audio::{AudioFormat as SpaAudioFormat, AudioInfoRaw};
use pipewire::spa::param::format::{MediaSubtype, MediaType};
use pipewire::spa::pod::Pod;
use pipewire::spa::utils::Direction as SpaDirection;
use pipewire::stream::{StreamFlags, StreamRc};
use pipewire::{context::ContextRc, main_loop::MainLoopRc, properties::properties};

use crate::backend::{AudioSource, RawAudio};
use crate::deliver::AudioQueue;
use crate::platform::linux::{now, quit_timer};

pub(crate) const BACKEND: &str = "pipewire-audio";

/// What the stream asks for. PipeWire converts in the graph, so a device running
/// at 44.1 kHz still arrives here at 48 kHz float, and every capturekit backend
/// then delivers the same shape.
const REQUESTED: AudioFormat = AudioFormat::STEREO_48K;

/// How long to wait for the daemon to answer a registry roundtrip.
const ENUMERATE_TIMEOUT: Duration = Duration::from_secs(2);

/// How many samples may queue before a stalled consumer starts losing them.
/// Four seconds of 48 kHz stereo float, which is far past any read interval a
/// recorder uses and still bounded.
const QUEUE_BYTES: usize = 4 * 48_000 * 2 * 4;

/// How long to wait for the stream to connect before calling the open failed.
const OPEN_TIMEOUT: Duration = Duration::from_secs(5);

fn failed(message: String) -> CaptureError {
    CaptureError::backend(BACKEND, std::io::Error::other(message))
}

/// A node as the registry described it.
struct Node {
    name: String,
    description: String,
    /// `Audio/Source` is a capture device; `Audio/Sink` is an output whose
    /// monitor is what a loopback reads.
    direction: AudioDirection,
}

/// Every audio node the daemon knows about.
///
/// Unlike ScreenCaptureKit, PipeWire has a real object registry, so this is a
/// list rather than a refusal. A sink is reported as a loopback device because
/// capturing one means reading its monitor, which is what
/// [`AudioDirection::Loopback`] names.
pub(crate) fn devices() -> Result<Vec<AudioDevice>> {
    let found = Arc::new(Mutex::new(Vec::new()));
    enumerate(&found).map_err(failed)?;
    let nodes = found
        .lock()
        .map_err(|_| failed("the registry listener panicked".into()))?;

    let mut devices = Vec::with_capacity(nodes.len());
    let mut seen_default = (false, false);
    for node in nodes.iter() {
        // The daemon lists in creation order and names no default through the
        // registry, so the first of each direction is the best answer available
        // without a session manager round trip.
        let is_default = match node.direction {
            AudioDirection::Input => !core::mem::replace(&mut seen_default.0, true),
            AudioDirection::Loopback => !core::mem::replace(&mut seen_default.1, true),
        };
        devices.push(AudioDevice {
            id: AudioDeviceId(node.name.clone()),
            name: node.description.clone(),
            direction: node.direction,
            is_default,
            // What the graph will convert to, not what the hardware runs at:
            // nothing here opens the device to ask, and reporting a guess about
            // the hardware would be worse than reporting what is delivered.
            format: REQUESTED,
        });
    }
    Ok(devices)
}

/// Run a registry roundtrip on a throwaway loop, collecting audio nodes.
fn enumerate(found: &Arc<Mutex<Vec<Node>>>) -> core::result::Result<(), String> {
    pipewire::init();
    let main_loop = MainLoopRc::new(None).map_err(|e| e.to_string())?;
    let context = ContextRc::new(&main_loop, None).map_err(|e| e.to_string())?;
    let core = context.connect_rc(None).map_err(|e| e.to_string())?;
    let registry = core.get_registry_rc().map_err(|e| e.to_string())?;

    let _listener = registry
        .add_listener_local()
        .global({
            let found = Arc::clone(found);
            move |global| {
                let Some(props) = global.props else { return };
                let Some(node) = node_of(props) else { return };
                if let Ok(mut found) = found.lock() {
                    found.push(node);
                }
            }
        })
        .register();

    // The daemon replays every existing object before answering a sync, so one
    // roundtrip is the whole list rather than a race against a timer.
    let done = Arc::new(AtomicBool::new(false));
    let _core_listener = core
        .add_listener_local()
        .done({
            let done = Arc::clone(&done);
            move |id, _seq| {
                if id == pipewire::core::PW_ID_CORE {
                    done.store(true, Ordering::Release);
                }
            }
        })
        .register();
    core.sync(0).map_err(|e| e.to_string())?;

    let _timer = quit_timer(&main_loop, &done, ENUMERATE_TIMEOUT)?;
    main_loop.run();
    Ok(())
}

/// Read a node out of the registry properties, or `None` when it is not audio.
fn node_of(props: &pipewire::spa::utils::dict::DictRef) -> Option<Node> {
    let class = props.get(*pipewire::keys::MEDIA_CLASS)?;
    let direction = match class {
        "Audio/Source" => AudioDirection::Input,
        "Audio/Sink" => AudioDirection::Loopback,
        _ => return None,
    };
    let name = props.get(*pipewire::keys::NODE_NAME)?.to_string();
    let description = props
        .get(*pipewire::keys::NODE_DESCRIPTION)
        .unwrap_or(&name)
        .to_string();
    Some(Node {
        name,
        description,
        direction,
    })
}

/// Where the stream is on its own sample timeline.
///
/// The pts of a buffer is the sample count before it, NOT the clock reading
/// when it happened to arrive. A delivery thread is late by a variable amount,
/// so stamping arrival leaves a hole wherever it was slower than usual, and a
/// consumer that lines buffers up end to end then hears a gap that is not in
/// the samples.
struct Timeline {
    format: AudioFormat,
    /// Read from the frame clock at the first buffer, so the audio and video
    /// tracks of one session share an origin.
    origin: Option<Timestamp>,
    counted: AudioTimeline,
}

impl Timeline {
    fn new(format: AudioFormat) -> Self {
        Self {
            format,
            origin: None,
            counted: AudioTimeline::new(format),
        }
    }

    /// Follow a renegotiation without letting the timeline jump backwards: the
    /// sample count restarts, so the origin moves forward by what it had run.
    fn renegotiated(&mut self, format: AudioFormat) {
        if format == self.format {
            return;
        }
        if let Some(origin) = self.origin.as_mut() {
            *origin = origin.saturating_add(self.counted.elapsed());
        }
        self.format = format;
        self.counted = AudioTimeline::new(format);
    }

    /// The instant these samples begin, and the timeline moved past them.
    fn stamp(&mut self, frames: u64) -> Timestamp {
        let origin = *self.origin.get_or_insert_with(now);
        let pts = origin.saturating_add(self.counted.elapsed());
        self.counted.advance(frames);
        pts
    }
}

/// A microphone or a sink's monitor, read through PipeWire.
pub(crate) struct PipewireAudioSource {
    queue: Arc<AudioQueue>,
    timeline: Arc<Mutex<Timeline>>,
    quit: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    desc: AudioDesc,
    current: Vec<u8>,
}

impl PipewireAudioSource {
    pub(crate) fn open(device: Option<&AudioDeviceId>, direction: AudioDirection) -> Result<Self> {
        // PipeWire's AUTOCONNECT falls back to the session manager's default
        // when a named target does not exist, so an unknown device would
        // silently record a different one. Checked here rather than trusted.
        if let Some(named) = device {
            let known = devices()?;
            if !known
                .iter()
                .any(|found| found.id == *named && found.direction == direction)
            {
                return Err(CaptureError::NotFoundNamed {
                    kind: "audio device",
                    id: named.0.clone(),
                });
            }
        }

        let queue = Arc::new(AudioQueue::default());
        let quit = Arc::new(AtomicBool::new(false));
        let negotiated = Arc::new(Mutex::new(Timeline::new(REQUESTED)));
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<core::result::Result<(), String>>();

        let thread = std::thread::Builder::new()
            .name("capturekit-pw-audio".into())
            .spawn({
                let queue = Arc::clone(&queue);
                let quit = Arc::clone(&quit);
                let timeline = Arc::clone(&negotiated);
                let target = device.map(|id| id.0.clone());
                move || {
                    let outcome =
                        run_stream(target, direction, &queue, &quit, &timeline, &ready_tx);
                    // Only a failure is news: a connected stream already reported.
                    let _ = ready_tx.send(outcome.clone());
                    if let Err(message) = outcome {
                        log::error!("pipewire audio ended: {message}");
                    }
                    queue.end();
                }
            })
            .map_err(|error| CaptureError::backend(BACKEND, error))?;

        // The signal means CONNECTED: anything else hands back a silent, empty track.
        match ready_rx.recv_timeout(OPEN_TIMEOUT) {
            Ok(Ok(())) => {}
            Ok(Err(message)) => return Err(failed(message)),
            Err(RecvTimeoutError::Timeout) => {
                return Err(failed(format!(
                    "the audio stream did not connect within {OPEN_TIMEOUT:?}"
                )))
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err(failed(
                    "the audio thread stopped before the stream connected".to_string(),
                ))
            }
        }

        Ok(Self {
            queue,
            timeline: negotiated,
            quit,
            thread: Some(thread),
            desc: AudioDesc {
                format: REQUESTED,
                device: AudioDeviceId(
                    device.map_or_else(|| "default".to_string(), |id| id.0.clone()),
                ),
                direction,
                backend: BACKEND,
            },
            current: Vec::new(),
        })
    }
}

/// Drive a PipeWire audio stream on this thread until asked to quit.
fn run_stream(
    target: Option<String>,
    direction: AudioDirection,
    queue: &Arc<AudioQueue>,
    quit: &Arc<AtomicBool>,
    timeline: &Arc<Mutex<Timeline>>,
    ready: &std::sync::mpsc::Sender<core::result::Result<(), String>>,
) -> core::result::Result<(), String> {
    pipewire::init();
    let main_loop = MainLoopRc::new(None).map_err(|e| e.to_string())?;
    let context = ContextRc::new(&main_loop, None).map_err(|e| e.to_string())?;
    let core = context.connect_rc(None).map_err(|e| e.to_string())?;

    let mut props = properties! {
        *pipewire::keys::MEDIA_TYPE => "Audio",
        *pipewire::keys::MEDIA_CATEGORY => "Capture",
        *pipewire::keys::MEDIA_ROLE => "Production",
    };
    if direction == AudioDirection::Loopback {
        // Capturing an output means reading its monitor, and this is the flag
        // that says so. Without it the stream connects to a source and a
        // loopback silently records the microphone instead.
        props.insert(*pipewire::keys::STREAM_CAPTURE_SINK, "true");
    }
    if let Some(target) = target {
        props.insert(*pipewire::keys::TARGET_OBJECT, target);
    }

    let stream = StreamRc::new(core, "capturekit", props).map_err(|e| e.to_string())?;

    let _listener = stream
        .add_local_listener_with_user_data(())
        .param_changed({
            let timeline = Arc::clone(timeline);
            move |_stream, (), id, param| {
                let Some(param) = param else { return };
                if id != pipewire::spa::param::ParamType::Format.as_raw() {
                    return;
                }
                let Ok((media_type, media_subtype)) =
                    pipewire::spa::param::format_utils::parse_format(param)
                else {
                    return;
                };
                if media_type != MediaType::Audio || media_subtype != MediaSubtype::Raw {
                    return;
                }
                let mut info = AudioInfoRaw::new();
                if info.parse(param).is_err() {
                    return;
                }
                let Some(format) = format_of(&info) else {
                    return;
                };
                if let Ok(mut timeline) = timeline.lock() {
                    timeline.renegotiated(format);
                }
            }
        })
        .process({
            let queue = Arc::clone(queue);
            let timeline = Arc::clone(timeline);
            move |stream, ()| {
                let Some(mut buffer) = stream.dequeue_buffer() else {
                    queue.note_dropped("the daemon offered no buffer to dequeue");
                    return;
                };
                let Some(data) = buffer.datas_mut().first_mut() else {
                    queue.note_dropped("a buffer arrived carrying no data plane");
                    return;
                };
                let chunk = data.chunk();
                let (offset, size) = (chunk.offset() as usize, chunk.size() as usize);
                let Some(bytes) = data.data() else {
                    queue.note_dropped("a buffer's data plane could not be mapped");
                    return;
                };
                let Some(samples) = bytes.get(offset..offset.saturating_add(size)) else {
                    queue.note_dropped("a buffer's chunk ran past the data it points into");
                    return;
                };
                let Ok(mut timeline) = timeline.lock() else {
                    queue.note_dropped("the stream timeline was poisoned by a panic");
                    return;
                };
                let format = timeline.format;
                if samples.is_empty() {
                    return;
                }
                // A partial sample frame means the negotiated channel count and
                // the buffer disagree; publishing it swaps the channels of
                // everything after it rather than failing.
                if let Err(err) = format.validate_buffer(samples.len()) {
                    queue.note_dropped_with("this stream's channel layout is unreadable", &err);
                    return;
                }
                let pts = timeline.stamp(format.frames_in(samples.len()) as u64);
                queue.publish(pts, samples, QUEUE_BYTES);
            }
        })
        .register()
        .map_err(|e| e.to_string())?;

    let pod_bytes = audio_params()?;
    let pod = Pod::from_bytes(&pod_bytes).ok_or("the audio format pod did not parse")?;
    stream
        .connect(
            SpaDirection::Input,
            None,
            StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS | StreamFlags::RT_PROCESS,
            &mut [pod],
        )
        .map_err(|e| e.to_string())?;

    let _timer = quit_timer(&main_loop, quit, Duration::from_millis(50))?;
    // Last, so nothing that can still fail reports success to the caller.
    let _ = ready.send(Ok(()));
    main_loop.run();
    Ok(())
}

/// Translate a negotiated PipeWire format, or `None` for one capturekit cannot
/// name.
fn format_of(info: &AudioInfoRaw) -> Option<AudioFormat> {
    // Only the interleaved little-endian layouts are asked for below, so
    // anything else means the graph negotiated something this cannot read.
    let sample_format = match info.format() {
        SpaAudioFormat::F32LE => SampleFormat::F32,
        SpaAudioFormat::S16LE => SampleFormat::I16,
        SpaAudioFormat::S32LE => SampleFormat::I32,
        _ => return None,
    };
    let channels = u16::try_from(info.channels()).ok()?;
    if channels == 0 || info.rate() == 0 {
        return None;
    }
    Some(AudioFormat::new(info.rate(), channels, sample_format))
}

/// Ask for interleaved 32-bit float, the layout the graph mixes in.
///
/// Offered as a single format rather than a choice: PipeWire converts for free,
/// and every other capturekit backend delivers interleaved samples, so accepting
/// a planar layout here would put the burden on every consumer instead.
fn audio_params() -> core::result::Result<Vec<u8>, String> {
    use pipewire::spa::pod::serialize::PodSerializer;
    use pipewire::spa::pod::Value;

    let mut info = AudioInfoRaw::new();
    info.set_format(SpaAudioFormat::F32LE);
    info.set_rate(REQUESTED.sample_rate);
    info.set_channels(u32::from(REQUESTED.channels));

    let values: Vec<u8> = PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &Value::Object(pipewire::spa::pod::Object {
            type_: pipewire::spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
            id: pipewire::spa::param::ParamType::EnumFormat.as_raw(),
            properties: info.into(),
        }),
    )
    .map(|(cursor, _)| cursor.into_inner())
    .map_err(|error| format!("building the audio format pod: {error}"))?;
    Ok(values)
}

impl Drop for PipewireAudioSource {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

impl AudioSource for PipewireAudioSource {
    fn describe(&self) -> &AudioDesc {
        &self.desc
    }

    fn next_buffer(&mut self, timeout: Duration) -> Result<RawAudio<'_>> {
        self.queue.report_drops(BACKEND);
        let (pts, lost) = self.queue.take(timeout, &mut self.current)?;
        // What the graph settled on, which is only knowable once it has.
        if let Ok(timeline) = self.timeline.lock() {
            self.desc.format = timeline.format;
        }
        Ok(RawAudio {
            pts,
            bytes: &self.current,
            // PipeWire drives capture from the graph clock, so an idle sink
            // still delivers buffers of real silence rather than nothing.
            silence: false,
            // Set only when the queue had to refuse samples, which is a real
            // break in the stream rather than a device hiccup.
            discontinuous: lost,
        })
    }

    fn stop(&mut self) -> Result<()> {
        self.quit.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        Ok(())
    }
}
