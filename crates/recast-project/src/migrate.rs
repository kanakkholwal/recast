//! v1 and v2 bundles to a v3 directory: a chain of pure steps over in-memory values, media touched once at the end.
//! Deterministic (ids come from the bundle's own bytes), lossless (unknown keys land under `<unknowns>` and are reported), atomic (a staging directory renamed into place).

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use recast_scene::v1::RenderState;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::document::Document;
use crate::ids::IdGen;
use crate::layout::{self, locate, Layout, Located};
use crate::scene::{
    from_render_state_public as from_render_state, MediaFile, MediaRefs, TrackFile,
    PLACED_PASSTHROUGH,
};
use crate::serialize::serialize;
use crate::validate::validate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// Keep the source bundle as `Name.recast.bak` beside the directory.
    pub keep_backup: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { keep_backup: true }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub from_version: u32,
    pub dest: PathBuf,
    pub backup: Option<PathBuf>,
    pub media: Vec<String>,
    pub tracks: Vec<String>,
    /// Keys the state carried that the document has no home for; preserved under `<unknowns>`.
    pub unknown_keys: Vec<String>,
    /// Validator warnings on the produced document, each a complete sentence.
    pub warnings: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum MigrateError {
    #[error(transparent)]
    Locate(#[from] layout::LocateError),
    #[error("{0} is already a v3 project")]
    AlreadyV3(PathBuf),
    #[error("{0} exists; refusing to overwrite a project")]
    DestExists(PathBuf),
    #[error("bundle entry '{0}' is missing")]
    MissingEntry(String),
    #[error("bundle entry '{entry}' is not valid: {message}")]
    BadEntry { entry: String, message: String },
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: io::Error,
    },
}

fn io_err(context: impl Into<String>) -> impl FnOnce(io::Error) -> MigrateError {
    let context = context.into();
    move |source| MigrateError::Io { context, source }
}

/// Migrates the bundle at `src` into the directory `dest`. When `dest` is the same path as `src`, the bundle is moved aside first.
/// # Errors On an unrecognised source, an existing destination, a corrupt bundle, or any filesystem failure; nothing is left half-written.
pub fn migrate(src: &Path, dest: &Path, opts: Options) -> Result<Report, MigrateError> {
    let (from_version, zip_path) = match locate(src)? {
        Located::V1Zip(p) => (1, p),
        Located::V2Zip(p) => (2, p),
        Located::V3Dir(p) | Located::Packaged(p) => return Err(MigrateError::AlreadyV3(p)),
    };
    let same_path = same_file(src, dest);
    if dest.exists() && !same_path {
        return Err(MigrateError::DestExists(dest.to_path_buf()));
    }

    let bundle = Bundle::open(&zip_path, from_version)?;
    let staging = staging_dir(dest);
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(io_err("clearing a stale staging directory"))?;
    }
    let outcome = write_project(&bundle, &staging, from_version);
    if outcome.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    let mut report = outcome?;

    let backup = if same_path {
        let bak = backup_path(src);
        fs::rename(src, &bak).map_err(io_err("moving the bundle aside"))?;
        Some(bak)
    } else {
        None
    };
    fs::rename(&staging, dest).map_err(io_err("renaming the staging directory into place"))?;
    if !opts.keep_backup {
        if let Some(bak) = &backup {
            let _ = fs::remove_file(bak);
        }
    }
    report.backup = backup.filter(|_| opts.keep_backup);
    report.dest = dest.to_path_buf();
    Ok(report)
}

fn same_file(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    norm(a) == norm(b)
}

fn staging_dir(dest: &Path) -> PathBuf {
    let mut name = dest
        .file_name()
        .map_or_else(|| "project".into(), |n| n.to_os_string());
    name.push(".migrating");
    dest.with_file_name(name)
}

fn backup_path(src: &Path) -> PathBuf {
    let mut name = src
        .file_name()
        .map_or_else(|| "project".into(), |n| n.to_os_string());
    name.push(".bak");
    src.with_file_name(name)
}

/// The bundle's parts as bytes and parsed JSON, read once from the archive.
struct Bundle {
    zip_path: PathBuf,
    edits: Value,
    metadata: Option<Value>,
    cursor: Option<Value>,
    media: Vec<(String, String)>,
}

impl Bundle {
    fn open(zip_path: &Path, version: u32) -> Result<Self, MigrateError> {
        let file = File::open(zip_path).map_err(io_err("opening the bundle"))?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| MigrateError::BadEntry {
            entry: "central directory".into(),
            message: e.to_string(),
        })?;
        let names: Vec<String> = archive.file_names().map(str::to_owned).collect();
        let mut read_json = |name: &str| -> Result<Option<Value>, MigrateError> {
            if !names.iter().any(|n| n == name) {
                return Ok(None);
            }
            let mut entry = archive.by_name(name).map_err(|e| MigrateError::BadEntry {
                entry: name.to_owned(),
                message: e.to_string(),
            })?;
            let mut text = String::new();
            entry
                .read_to_string(&mut text)
                .map_err(io_err(format!("reading {name}")))?;
            serde_json::from_str(&text)
                .map(Some)
                .map_err(|e| MigrateError::BadEntry {
                    entry: name.to_owned(),
                    message: e.to_string(),
                })
        };
        let edits = if version == 1 {
            read_json("edits.json")?
                .ok_or_else(|| MigrateError::MissingEntry("edits.json".into()))?
        } else {
            let manifest = read_json("project.json")?
                .ok_or_else(|| MigrateError::MissingEntry("project.json".into()))?;
            let mut merged = Map::new();
            for entry in manifest
                .get("edits")
                .and_then(Value::as_object)
                .into_iter()
                .flat_map(|m| m.values())
            {
                let Some(section) = entry.as_str().map(&mut read_json).transpose()?.flatten()
                else {
                    continue;
                };
                for (k, v) in section.as_object().into_iter().flat_map(|m| m.iter()) {
                    if k != "version" {
                        merged.insert(k.clone(), v.clone());
                    }
                }
            }
            Value::Object(merged)
        };
        let metadata = read_json("metadata.json")?;
        let cursor = if version == 1 {
            read_json("cursor.json")?
        } else {
            read_json("assets/cursor.track.json")?
        };
        let media = media_entries(version)
            .into_iter()
            .filter(|(entry, _)| names.iter().any(|n| n == entry))
            .map(|(e, t)| (e.to_owned(), t.to_owned()))
            .collect();
        Ok(Self {
            zip_path: zip_path.to_path_buf(),
            edits,
            metadata,
            cursor,
            media,
        })
    }
}

/// Archive entry to document-relative destination, per bundle version.
fn media_entries(version: u32) -> Vec<(&'static str, &'static str)> {
    if version == 1 {
        vec![
            ("recording.mp4", layout::RECORDING),
            ("camera.mp4", layout::CAMERA),
            ("microphone.wav", layout::MIC),
            ("audio.wav", layout::SYSTEM),
        ]
    } else {
        vec![
            ("assets/recording.mp4", layout::RECORDING),
            ("assets/camera.mp4", layout::CAMERA),
            ("assets/microphone.wav", layout::MIC),
            ("assets/audio.wav", layout::SYSTEM),
        ]
    }
}

fn write_project(
    bundle: &Bundle,
    staging: &Path,
    from_version: u32,
) -> Result<Report, MigrateError> {
    let layout = Layout::new(staging);
    for dir in [
        layout::MEDIA_DIR,
        layout::TRACKS_DIR,
        layout::BRANCHES_DIR,
        layout::CACHE_DIR,
    ] {
        fs::create_dir_all(staging.join(dir))
            .map_err(io_err("creating the project directories"))?;
    }
    let mut state: RenderState =
        serde_json::from_value(bundle.edits.clone()).map_err(|e| MigrateError::BadEntry {
            entry: "edits".into(),
            message: e.to_string(),
        })?;
    let mut tracks = Vec::new();
    let words = state
        .passthrough
        .remove("transcript")
        .filter(|t| !t.is_null());
    let words_ref = words
        .as_ref()
        .map(|t| write_words(&layout, t))
        .transpose()?
        .inspect(|_| tracks.push(layout::WORDS_TRACK.to_owned()));
    let cursor_ref = bundle
        .cursor
        .as_ref()
        .map(|c| write_cursor(&layout, c))
        .transpose()?
        .inspect(|_| tracks.push(layout::CURSOR_TRACK.to_owned()));

    let media = extract_media(bundle, &layout)?;
    if let Some(meta) = &bundle.metadata {
        write_capture(&layout, meta)?;
    }
    let refs = media_refs(bundle, &media, cursor_ref, words_ref);

    let seed = Sha256::digest(bundle.edits.to_string().as_bytes());
    let mut ids = IdGen::seeded(u64::from_le_bytes(seed[..8].try_into().unwrap_or([0; 8])));
    let doc: Document = from_render_state(&state, &refs, None, &mut ids);
    // A value the editor wrote is never a reason to strand the project: every finding goes in the report, the document still opens.
    let report = validate(&doc);
    write_atomic(&layout.document(), serialize(&doc).as_bytes())?;

    Ok(Report {
        from_version,
        dest: staging.to_path_buf(),
        backup: None,
        media,
        tracks,
        unknown_keys: state
            .passthrough
            .keys()
            .filter(|k| !PLACED_PASSTHROUGH.contains(&k.as_str()))
            .cloned()
            .collect(),
        warnings: report
            .issues
            .iter()
            .map(|i| format!("{}: {}", i.element, i.message))
            .collect(),
    })
}

fn write_words(layout: &Layout, transcript: &Value) -> Result<TrackFile, MigrateError> {
    let words: Vec<(f64, f64)> = transcript
        .get("segments")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|s| {
            s.get("words")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter_map(|w| Some((w.get("start")?.as_f64()?, w.get("end")?.as_f64()?)))
        .collect();
    write_atomic(
        &layout.resolve(layout::WORDS_TRACK),
        transcript.to_string().as_bytes(),
    )?;
    Ok(TrackFile {
        src: layout::WORDS_TRACK.into(),
        rows: words.len(),
        span: words.first().zip(words.last()).map(|(f, l)| (f.0, l.1)),
        engine: transcript
            .get("engine")
            .and_then(Value::as_str)
            .map(str::to_owned),
        model: transcript
            .get("modelId")
            .and_then(Value::as_str)
            .map(str::to_owned),
        lang: transcript
            .get("language")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

/// Velocities are derivable and read by nothing, so the track shrinks by dropping them; everything else is kept as captured.
fn write_cursor(layout: &Layout, track: &Value) -> Result<TrackFile, MigrateError> {
    let mut compact = track.clone();
    let mut rows = 0;
    let mut span = None;
    if let Some(samples) = compact.get_mut("samples").and_then(Value::as_array_mut) {
        rows = samples.len();
        let ts = |s: &Value| {
            s.get("timestampUs")
                .and_then(Value::as_f64)
                .map(|us| us / 1e6)
        };
        span = samples
            .first()
            .and_then(ts)
            .zip(samples.last().and_then(ts));
        for sample in samples.iter_mut() {
            if let Some(obj) = sample.as_object_mut() {
                obj.remove("velocityX");
                obj.remove("velocityY");
            }
        }
    }
    write_atomic(
        &layout.resolve(layout::CURSOR_TRACK),
        compact.to_string().as_bytes(),
    )?;
    Ok(TrackFile {
        src: layout::CURSOR_TRACK.into(),
        rows,
        span,
        engine: None,
        model: None,
        lang: None,
    })
}

fn write_capture(layout: &Layout, meta: &Value) -> Result<(), MigrateError> {
    let text = serde_json::to_string_pretty(meta).map_err(|e| MigrateError::BadEntry {
        entry: "metadata.json".into(),
        message: e.to_string(),
    })?;
    write_atomic(&layout.capture(), text.as_bytes())
}

fn extract_media(bundle: &Bundle, layout: &Layout) -> Result<Vec<String>, MigrateError> {
    let file = File::open(&bundle.zip_path).map_err(io_err("opening the bundle"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| MigrateError::BadEntry {
        entry: "central directory".into(),
        message: e.to_string(),
    })?;
    let mut written = Vec::new();
    for (entry, target) in &bundle.media {
        let mut source = archive.by_name(entry).map_err(|e| MigrateError::BadEntry {
            entry: entry.clone(),
            message: e.to_string(),
        })?;
        let path = layout.resolve(target);
        let mut out = File::create(&path).map_err(io_err(format!("creating {target}")))?;
        io::copy(&mut source, &mut out).map_err(io_err(format!("extracting {entry}")))?;
        out.sync_all()
            .map_err(io_err(format!("flushing {target}")))?;
        written.push(target.clone());
    }
    Ok(written)
}

fn media_refs(
    bundle: &Bundle,
    media: &[String],
    cursor: Option<TrackFile>,
    words: Option<TrackFile>,
) -> MediaRefs {
    let meta = bundle.metadata.as_ref();
    let num = |path: &[&str]| -> f64 {
        let mut at = meta;
        for key in path {
            at = at.and_then(|v| v.get(key));
        }
        at.and_then(Value::as_f64).unwrap_or(0.0)
    };
    let has = |rel: &str| media.iter().any(|m| m == rel);
    MediaRefs {
        recording: has(layout::RECORDING).then(|| MediaFile {
            src: layout::RECORDING.into(),
            width: num(&["video", "width"]) as u32,
            height: num(&["video", "height"]) as u32,
            fps: num(&["video", "fps"]),
            duration: num(&["video", "durationMs"]) / 1000.0,
            offset: 0.0,
        }),
        camera: has(layout::CAMERA).then(|| MediaFile {
            src: layout::CAMERA.into(),
            offset: num(&["media", "trackOffsets", "cameraMs"]) / 1000.0,
            ..Default::default()
        }),
        mic: has(layout::MIC).then(|| layout::MIC.into()),
        system: has(layout::SYSTEM).then(|| layout::SYSTEM.into()),
        cursor,
        words,
    }
}

/// Temp beside the target, fsync, rename: a crash leaves the old file or the new one, never a torn one.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), MigrateError> {
    let tmp = path.with_extension("tmp");
    {
        let mut file = File::create(&tmp).map_err(io_err(format!("creating {}", tmp.display())))?;
        io::Write::write_all(&mut file, bytes)
            .map_err(io_err(format!("writing {}", tmp.display())))?;
        file.sync_all()
            .map_err(io_err(format!("flushing {}", tmp.display())))?;
    }
    fs::rename(&tmp, path).map_err(io_err(format!("renaming into {}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn v2_bundle(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        let mut w = zip::ZipWriter::new(File::create(&path).unwrap());
        let o = zip::write::SimpleFileOptions::default();
        let mut put = |n: &str, body: &str| {
            w.start_file(n, o).unwrap();
            w.write_all(body.as_bytes()).unwrap();
        };
        put(
            "project.json",
            r##"{"format":"recast-project","formatVersion":2,"capture":"metadata.json","assets":{"video":"assets/recording.mp4","cursorTrack":"assets/cursor.track.json","microphone":"assets/microphone.wav"},"edits":{"frame":"edits/frame.json","cursor":"edits/cursor.json","timeline":"edits/timeline.json","zoom":"edits/zoom.json"}}"##,
        );
        put(
            "edits/cursor.json",
            r##"{"version":1,"cursorEnabled":true,"cursorSize":3,"cursorSmoothing":50,"cursorHighlightClicks":true,"cursorHighlightColor":"#3b82f6","cursorHighlightOpacity":40,"cursorHideWhenIdle":false,"cursorIdleTimeout":3,"cursorStyle":"dot"}"##,
        );
        put(
            "metadata.json",
            r##"{"schemaVersion":1,"video":{"width":1920,"height":1080,"fps":60,"durationMs":11350},"media":{"hasMicrophone":true,"trackOffsets":{"microphoneMs":680}}}"##,
        );
        put("assets/recording.mp4", "MP4BYTES");
        put("assets/microphone.wav", "WAVBYTES");
        put(
            "assets/cursor.track.json",
            r##"{"samples":[{"timestampUs":717,"x":354,"y":764,"velocityX":0.0,"velocityY":0.0,"visible":true},{"timestampUs":8952,"x":354,"y":755,"velocityX":1.0,"velocityY":-2.0,"visible":true}],"clicks":[]}"##,
        );
        put(
            "edits/frame.json",
            r##"{"version":1,"trimStart":0,"trimEnd":11.35,"padding":40,"backgroundType":"color","backgroundValue":"#111111","backgroundBlur":0,"borderRadius":0,"transcript":{"engine":"parakeet","modelId":"v3","language":"en","segments":[{"id":"s1","start":1,"end":2,"text":"hi there","words":[{"start":1.0,"end":1.4,"text":"hi"},{"start":1.5,"end":2.0,"text":"there"}]}]},"someFutureToggle":123}"##,
        );
        put(
            "edits/timeline.json",
            r##"{"version":1,"cuts":[{"id":"tr703cp","source":"manual","start":1.8365,"end":3.1603}],"cutsEnabled":true,"splitPoints":[]}"##,
        );
        put(
            "edits/zoom.json",
            r##"{"version":1,"zoomRegions":[],"autoZoomEnabled":true}"##,
        );
        w.finish().unwrap();
        path
    }

    #[test]
    fn a_v2_bundle_becomes_a_directory_with_document_media_tracks_and_capture() {
        let tmp = tempfile::tempdir().unwrap();
        let src = v2_bundle(tmp.path(), "Take.recast");
        let report = migrate(&src, &src, Options::default()).unwrap();
        assert_eq!(report.from_version, 2);
        assert!(src.is_dir(), "the directory takes the bundle's name");
        assert_eq!(
            report.backup.as_deref(),
            Some(tmp.path().join("Take.recast.bak").as_path())
        );
        assert!(report.backup.as_ref().unwrap().is_file());
        assert_eq!(fs::read(src.join(layout::RECORDING)).unwrap(), b"MP4BYTES");
        assert!(src.join(layout::MIC).is_file());
        assert!(src.join(layout::CAPTURE).is_file());
        let cursor: Value =
            serde_json::from_slice(&fs::read(src.join(layout::CURSOR_TRACK)).unwrap()).unwrap();
        assert!(
            cursor["samples"][0].get("velocityX").is_none(),
            "velocities are stripped"
        );
        let words: Value =
            serde_json::from_slice(&fs::read(src.join(layout::WORDS_TRACK)).unwrap()).unwrap();
        assert_eq!(words["segments"][0]["words"].as_array().unwrap().len(), 2);
        let text = fs::read_to_string(src.join(layout::DOCUMENT)).unwrap();
        assert!(text.contains("<media id=\"rec\" kind=\"video\" src=\"media/recording.mp4\" w=\"1920\" h=\"1080\" fps=\"60\" dur=\"11.350\"/>"), "{text}");
        assert!(text.contains("<track id=\"wrd\" kind=\"words\" src=\"tracks/words.json\" n=\"2\" span=\"1.000 2.000\" engine=\"parakeet\" model=\"v3\" lang=\"en\"/>"), "{text}");
        assert!(
            text.contains("<cut id=\"tr703cp\" at=\"1.837\" dur=\"1.324\"/>"),
            "{text}"
        );
        assert!(
            text.contains("<unknown key=\"someFutureToggle\" json=\"123\"/>"),
            "{text}"
        );
        assert_eq!(report.unknown_keys, vec!["someFutureToggle"]);
        assert!(
            !text.contains("transcript"),
            "the transcript left the document"
        );
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    }

    #[test]
    fn migrating_the_same_bundle_twice_yields_identical_documents() {
        let tmp = tempfile::tempdir().unwrap();
        let src = v2_bundle(tmp.path(), "Take.recast");
        let a = tmp.path().join("A.recast");
        let b = tmp.path().join("B.recast");
        migrate(&src, &a, Options::default()).unwrap();
        migrate(&src, &b, Options::default()).unwrap();
        assert_eq!(
            fs::read(a.join(layout::DOCUMENT)).unwrap(),
            fs::read(b.join(layout::DOCUMENT)).unwrap()
        );
        assert!(
            src.is_file(),
            "a migration to another path leaves the bundle alone"
        );
    }

    #[test]
    fn an_existing_destination_is_refused_and_nothing_is_staged() {
        let tmp = tempfile::tempdir().unwrap();
        let src = v2_bundle(tmp.path(), "Take.recast");
        let dest = tmp.path().join("Busy.recast");
        fs::create_dir(&dest).unwrap();
        assert!(matches!(
            migrate(&src, &dest, Options::default()).unwrap_err(),
            MigrateError::DestExists(_)
        ));
        assert!(!tmp.path().join("Busy.recast.migrating").exists());
    }

    #[test]
    fn a_v1_bundle_migrates_through_the_same_path() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("Old.recast");
        let mut w = zip::ZipWriter::new(File::create(&path).unwrap());
        let o = zip::write::SimpleFileOptions::default();
        for (n, body) in [
            (
                "edits.json",
                r##"{"trimStart":0,"trimEnd":5,"backgroundType":"color","backgroundValue":"#000000","backgroundBlur":0,"padding":0,"cursorEnabled":true,"cursorSize":3,"cursorSmoothing":50,"cursorHighlightClicks":true,"cursorHighlightColor":"#3b82f6","cursorHighlightOpacity":40,"cursorHideWhenIdle":false,"cursorIdleTimeout":3,"zoomRegions":[]}"##,
            ),
            ("cursor.json", r##"{"samples":[],"clicks":[]}"##),
            ("recording.mp4", "MP4"),
        ] {
            w.start_file(n, o).unwrap();
            w.write_all(body.as_bytes()).unwrap();
        }
        w.finish().unwrap();
        let report = migrate(&path, &path, Options { keep_backup: false }).unwrap();
        assert_eq!(report.from_version, 1);
        assert!(report.backup.is_none());
        assert!(!tmp.path().join("Old.recast.bak").exists());
        assert!(path.join(layout::DOCUMENT).is_file());
        assert!(path.join(layout::RECORDING).is_file());
    }

    /// The two real bundles in the owner's library; run by hand, never in CI.
    #[test]
    #[ignore = "needs the owner's real recordings"]
    fn the_real_bundles_migrate_cleanly() {
        for name in [
            "Recast_2026-08-29_20-30-54.recast",
            "Recast_2026-07-12_01-53-42.recast",
        ] {
            let src = PathBuf::from("C:/Users/kanak/Videos/Recast/recasts").join(name);
            let tmp = tempfile::tempdir().unwrap();
            let dest = tmp.path().join(name);
            let report = migrate(&src, &dest, Options::default()).unwrap();
            // Legacy values (a 4x zoom from before the slider capped at 3) are kept and reported, never a refusal.
            eprintln!("{name}: {:?}", report.warnings);
            let doc =
                crate::parse::parse(&fs::read_to_string(dest.join(layout::DOCUMENT)).unwrap())
                    .unwrap();
            crate::scene::to_scene(&doc).unwrap();
        }
    }
}
