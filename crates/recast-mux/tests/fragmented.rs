use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use recast_mux::{
    annex_b_to_avcc, split_access_units, AvcConfig, FragmentedWriter, Mp4Reader, VideoFormat,
};

const WIDTH: u16 = 160;
const HEIGHT: u16 = 120;
const FPS: u32 = 25;
const FRAMES: u32 = 50;
const TIMESCALE: u32 = 25;
/// Frames per fragment. Small, so a truncated file still has several.
const PER_FRAGMENT: usize = 10;

fn ffprobe() -> Option<PathBuf> {
    let ffmpeg = recast_testkit::ffmpeg_path()?;
    let name = ffmpeg.file_name()?.to_str()?.replacen("ffmpeg", "ffprobe", 1);
    let probe = ffmpeg.with_file_name(name);
    probe.exists().then_some(probe)
}

fn probe_field(ffprobe: &Path, file: &Path, entry: &str) -> String {
    let out = Command::new(ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            entry,
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(file)
        .stdin(Stdio::null())
        .output()
        .expect("ffprobe runs");
    String::from_utf8_lossy(&out.stdout)
        .replace("\r\n", "\n")
        .trim()
        .to_string()
}

struct Built {
    /// The whole file: initialisation segment then every fragment.
    data: Vec<u8>,
    /// Where the initialisation segment ends.
    init: usize,
    /// Byte length after each fragment, so a cut can land on a boundary.
    boundaries: Vec<usize>,
    ffprobe: PathBuf,
}

/// Memoised: several tests use it, and each rewriting the same path races.
fn built() -> Option<&'static Built> {
    static FILE: OnceLock<Option<Built>> = OnceLock::new();
    FILE.get_or_init(build).as_ref()
}

fn build() -> Option<Built> {
    let Some(ffmpeg) = recast_testkit::ffmpeg_path() else {
        eprintln!("skipping: no ffmpeg sidecar");
        return None;
    };
    let Some(ffprobe) = ffprobe() else {
        eprintln!("skipping: no ffprobe beside {}", ffmpeg.display());
        return None;
    };
    let dir = std::env::temp_dir().join("recast-mux-tests");
    std::fs::create_dir_all(&dir).ok()?;
    let raw = dir.join("fragment-source.h264");
    let ok = Command::new(&ffmpeg)
        .args(["-y", "-f", "lavfi", "-i"])
        .arg(format!("testsrc=size={WIDTH}x{HEIGHT}:rate={FPS}"))
        .args(["-frames:v", &FRAMES.to_string()])
        .args(["-c:v", "libx264", "-preset", "ultrafast", "-bf", "0"])
        .args(["-g", "10", "-f", "h264"])
        .arg(&raw)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        eprintln!("skipping: ffmpeg could not produce an elementary stream");
        return None;
    }
    let stream = std::fs::read(&raw).ok()?;

    let mut writer = FragmentedWriter::new(VideoFormat {
        width: WIDTH,
        height: HEIGHT,
        timescale: TIMESCALE,
    });
    // The parameter sets have to be known before the initialisation segment, so
    // the stream is walked once to find them.
    let mut config = AvcConfig::default();
    let mut samples = Vec::new();
    for unit in split_access_units(&stream) {
        let converted = annex_b_to_avcc(&unit);
        if !converted.config.is_empty() {
            config = converted.config;
        }
        if !converted.sample.is_empty() {
            samples.push((converted.sample, converted.is_sync));
        }
    }
    writer.set_avc_config(config);

    let mut data = writer.initialization_segment().ok()?;
    let init = data.len();
    let mut boundaries = Vec::new();
    for chunk in samples.chunks(PER_FRAGMENT) {
        for (sample, is_sync) in chunk {
            writer.push_sample(sample, 1, *is_sync);
        }
        data.extend_from_slice(&writer.fragment().ok()??);
        boundaries.push(data.len());
    }

    std::fs::write(dir.join("fragmented.mp4"), &data).ok()?;
    Some(Built {
        data,
        init,
        boundaries,
        ffprobe,
    })
}

fn write_temp(name: &str, data: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join("recast-mux-tests");
    std::fs::create_dir_all(&dir).expect("a temp dir");
    let path = dir.join(name);
    std::fs::write(&path, data).expect("the file writes");
    path
}

#[test]
fn a_fragmented_file_reads_as_h264_at_the_right_size() {
    let Some(built) = built() else { return };
    let path = write_temp("fragmented-whole.mp4", &built.data);
    assert_eq!(probe_field(&built.ffprobe, &path, "stream=codec_name"), "h264");
    assert_eq!(
        probe_field(&built.ffprobe, &path, "stream=width,height"),
        format!("{WIDTH}\n{HEIGHT}")
    );
}

/// Decoding is the real proof: every `trun` offset has to point at the right
/// bytes and `tfdt` has to put each fragment where it belongs.
#[test]
fn every_fragment_decodes_back_out() {
    let Some(built) = built() else { return };
    let path = write_temp("fragmented-whole.mp4", &built.data);
    let counted = decoded_frames(&built.ffprobe, &path).unwrap_or(0);
    assert_eq!(counted, FRAMES, "decoded {counted} frames");
}

fn decoded_frames(ffprobe: &Path, file: &Path) -> Option<u32> {
    let out = Command::new(ffprobe)
        .args([
            "-v",
            "error",
            "-count_frames",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=nb_read_frames",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(file)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

/// The whole reason for fragments. A progressive file cut short is unplayable,
/// because its header is written last. This one has to play up to the last
/// complete fragment.
#[test]
fn a_file_cut_short_still_plays_what_it_has() {
    let Some(built) = built() else { return };
    let boundary = built.boundaries[1];
    let path = write_temp("fragmented-cut.mp4", &built.data[..boundary]);

    assert_eq!(
        probe_field(&built.ffprobe, &path, "stream=codec_name"),
        "h264",
        "a truncated fragmented file did not even open"
    );
    let counted = decoded_frames(&built.ffprobe, &path).unwrap_or(0);
    let expected = (PER_FRAGMENT * 2) as u32;
    assert_eq!(
        counted, expected,
        "a file cut after two fragments decoded {counted} frames, wanted {expected}"
    );
}

/// Cutting mid-fragment is the realistic crash: the last `mdat` is short. What
/// came before it still has to play.
#[test]
fn a_file_cut_inside_a_fragment_keeps_the_ones_before_it() {
    let Some(built) = built() else { return };
    let cut = (built.boundaries[1] + built.boundaries[2]) / 2;
    let path = write_temp("fragmented-torn.mp4", &built.data[..cut]);
    let counted = decoded_frames(&built.ffprobe, &path).unwrap_or(0);
    assert!(
        counted >= (PER_FRAGMENT * 2) as u32,
        "a torn file lost the fragments before the tear: {counted} frames"
    );
}

/// The initialisation segment alone is a valid movie with no media, which is
/// what makes it safe to write before anything has been encoded.
#[test]
fn the_initialisation_segment_opens_on_its_own() {
    let Some(built) = built() else { return };
    let path = write_temp("fragmented-init.mp4", &built.data[..built.init]);
    assert_eq!(
        probe_field(&built.ffprobe, &path, "stream=codec_name"),
        "h264"
    );
}

/// Our own reader parses `moov`, which in a fragmented file describes the
/// tracks and holds no samples. Saying so here stops the empty list looking
/// like a parsing bug later.
#[test]
fn our_reader_sees_the_tracks_but_none_of_the_samples() {
    let Some(built) = built() else { return };
    let reader = Mp4Reader::new(&built.data).expect("the file parses");
    let video = reader.video().expect("a video track");
    assert_eq!(&video.format, b"avc1");
    assert_eq!((video.width, video.height), (WIDTH, HEIGHT));
    assert!(
        video.samples.is_empty(),
        "moov carried {} samples in a fragmented file",
        video.samples.len()
    );
}
