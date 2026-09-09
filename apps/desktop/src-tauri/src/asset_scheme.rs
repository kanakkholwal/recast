//! `recast-asset://`: the one file resolver the webview reads media through. Confined to roots the app owns plus
//! files a document names or the user picked; explicit MIME by extension; single-range reads for video seeking.

use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use parking_lot::RwLock;
use tauri::http::{header, HeaderValue, Request, Response, StatusCode};

pub const SCHEME: &str = "recast-asset";

/// The largest single range served in one response; a video element asks again for the rest.
const MAX_RANGE_BYTES: u64 = 1024 * 1024;

static SCOPE: LazyLock<Scope> = LazyLock::new(Scope::default);

pub fn scope() -> &'static Scope {
    &SCOPE
}

/// What may be served: whole directories the app owns, and single files granted one by one.
#[derive(Default)]
pub struct Scope {
    roots: RwLock<Vec<PathBuf>>,
    files: RwLock<HashSet<PathBuf>>,
}

impl Scope {
    pub fn allow_root(&self, dir: &Path) {
        let dir = normal(dir);
        let mut roots = self.roots.write();
        if !roots.contains(&dir) {
            roots.push(dir);
        }
    }

    pub fn grant_file(&self, file: &Path) {
        self.files.write().insert(normal(file));
    }

    /// Grants every absolute path to an existing file that `value` names, at any depth. The document authorises what it references.
    pub fn grant_named_in(&self, value: &serde_json::Value) {
        match value {
            serde_json::Value::String(s) => {
                let p = Path::new(s);
                if p.is_absolute() && p.is_file() {
                    self.grant_file(p);
                }
            }
            serde_json::Value::Array(items) => items.iter().for_each(|v| self.grant_named_in(v)),
            serde_json::Value::Object(map) => map.values().for_each(|v| self.grant_named_in(v)),
            _ => {}
        }
    }

    #[must_use]
    pub fn is_allowed(&self, file: &Path) -> bool {
        let file = normal(file);
        self.files.read().contains(&file) || self.roots.read().iter().any(|r| file.starts_with(r))
    }
}

/// Canonical form (symlinks, drive-letter case, the Windows verbatim prefix): the deepest existing ancestor canonicalised, the rest appended.
/// A missing file under a root must still compare equal to that root, or it answers 403 where 404 is the truth.
fn normal(path: &Path) -> PathBuf {
    let mut rest = Vec::new();
    let mut cursor = path;
    loop {
        if let Ok(canonical) = cursor.canonicalize() {
            return rest
                .iter()
                .rev()
                .fold(canonical, |acc, part| acc.join(part));
        }
        match (cursor.file_name(), cursor.parent()) {
            (Some(name), Some(parent)) => {
                rest.push(name.to_owned());
                cursor = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// Handles one request. Errors are HTTP statuses, never panics: the protocol thread must stay alive.
/// WebView2 runs CORS on a registered scheme, so every answer carries the allow headers and a preflight is answered here.
pub fn handle(request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let mut response = respond(request);
    let headers = response.headers_mut();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static("content-range, content-length, accept-ranges"),
    );
    response
}

fn respond(request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    if request.method() == tauri::http::Method::OPTIONS {
        return Response::builder()
            .status(StatusCode::NO_CONTENT)
            .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET, HEAD, OPTIONS")
            .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "range, content-type")
            .header(header::ACCESS_CONTROL_MAX_AGE, "86400")
            .body(Vec::new())
            .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR));
    }
    let Some(path) = path_of(request.uri().path()) else {
        return status(StatusCode::BAD_REQUEST);
    };
    if !scope().is_allowed(&path) {
        log::warn!("recast-asset refused {}", path.display());
        return status(StatusCode::FORBIDDEN);
    }
    let mut file = match File::open(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return status(StatusCode::NOT_FOUND),
        Err(_) => return status(StatusCode::FORBIDDEN),
    };
    let Ok(len) = file.metadata().map(|m| m.len()) else {
        return status(StatusCode::INTERNAL_SERVER_ERROR);
    };
    let mime = mime_of(&path);
    let range = request
        .headers()
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .map(|r| parse_range(r, len));
    match range {
        None => match read_span(&mut file, 0, len) {
            Ok(body) => ok(
                StatusCode::OK,
                mime,
                body,
                [(header::ACCEPT_RANGES, "bytes".to_owned())],
            ),
            Err(_) => status(StatusCode::INTERNAL_SERVER_ERROR),
        },
        Some(Some((start, end))) => match read_span(&mut file, start, end + 1 - start) {
            Ok(body) => ok(
                StatusCode::PARTIAL_CONTENT,
                mime,
                body,
                [
                    (header::ACCEPT_RANGES, "bytes".to_owned()),
                    (header::CONTENT_RANGE, format!("bytes {start}-{end}/{len}")),
                ],
            ),
            Err(_) => status(StatusCode::INTERNAL_SERVER_ERROR),
        },
        Some(None) => Response::builder()
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(header::CONTENT_RANGE, format!("bytes */{len}"))
            .body(Vec::new())
            .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR)),
    }
}

/// The file a request names. `convertFileSrc` percent-encodes the absolute path as the whole URL path.
fn path_of(uri_path: &str) -> Option<PathBuf> {
    let decoded = urlencoding::decode(uri_path.strip_prefix('/').unwrap_or(uri_path)).ok()?;
    let path = PathBuf::from(decoded.as_ref());
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return None;
    }
    Some(path)
}

/// One `bytes=` range clipped to `MAX_RANGE_BYTES`; `None` inside when it cannot be satisfied. Multi-range is served whole by the caller.
fn parse_range(header: &str, len: u64) -> Option<(u64, u64)> {
    let spec = header.strip_prefix("bytes=")?.trim();
    if spec.contains(',') || len == 0 {
        return None;
    }
    let (start, end) = spec.split_once('-')?;
    let (start, end) = match (start.trim(), end.trim()) {
        ("", suffix) => {
            let n: u64 = suffix.parse().ok()?;
            (len.saturating_sub(n.max(1)), len - 1)
        }
        (s, "") => (s.parse().ok()?, len - 1),
        (s, e) => (s.parse().ok()?, e.parse::<u64>().ok()?.min(len - 1)),
    };
    if start >= len || end < start {
        return None;
    }
    Some((start, end.min(start + MAX_RANGE_BYTES - 1)))
}

fn read_span(file: &mut File, start: u64, count: u64) -> std::io::Result<Vec<u8>> {
    file.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::with_capacity(usize::try_from(count).unwrap_or(0));
    file.take(count).read_to_end(&mut buf)?;
    Ok(buf)
}

/// Explicit types only: the webview refuses SVG (and fonts) under a sniffed or generic type.
fn mime_of(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("mp4" | "m4v") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mov") => "video/quicktime",
        Some("mkv") => "video/x-matroska",
        Some("wav") => "audio/wav",
        Some("mp3") => "audio/mpeg",
        Some("m4a" | "aac") => "audio/mp4",
        Some("ogg" | "oga") => "audio/ogg",
        Some("flac") => "audio/flac",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("svg") => "image/svg+xml",
        Some("json") => "application/json",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("vtt") => "text/vtt",
        Some("txt" | "srt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn ok<const N: usize>(
    code: StatusCode,
    mime: &str,
    body: Vec<u8>,
    extra: [(header::HeaderName, String); N],
) -> Response<Vec<u8>> {
    let mut builder = Response::builder()
        .status(code)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CONTENT_LENGTH, body.len())
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    for (name, value) in extra {
        builder = builder.header(name, value);
    }
    builder
        .body(body)
        .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR))
}

fn status(code: StatusCode) -> Response<Vec<u8>> {
    let mut response = Response::new(Vec::new());
    *response.status_mut() = code;
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get(url_path: &str, range: Option<&str>) -> Response<Vec<u8>> {
        let mut builder = Request::builder().uri(format!("recast-asset://localhost{url_path}"));
        if let Some(r) = range {
            builder = builder.header(header::RANGE, r);
        }
        handle(&builder.body(Vec::new()).unwrap())
    }

    fn encoded(path: &Path) -> String {
        format!("/{}", urlencoding::encode(&path.to_string_lossy()))
    }

    fn header_of(r: &Response<Vec<u8>>, name: header::HeaderName) -> &str {
        r.headers()
            .get(name)
            .map(|v| v.to_str().unwrap())
            .unwrap_or("")
    }

    #[test]
    fn a_file_outside_every_root_and_grant_is_refused_and_a_granted_one_is_served_typed() {
        let tmp = tempfile::tempdir().unwrap();
        let svg = tmp.path().join("arrow.svg");
        std::fs::write(&svg, "<svg/>").unwrap();
        assert_eq!(get(&encoded(&svg), None).status(), StatusCode::FORBIDDEN);
        scope().grant_file(&svg);
        let r = get(&encoded(&svg), None);
        assert_eq!(r.status(), StatusCode::OK);
        assert_eq!(header_of(&r, header::CONTENT_TYPE), "image/svg+xml");
        assert_eq!(header_of(&r, header::X_CONTENT_TYPE_OPTIONS), "nosniff");
        assert_eq!(r.body(), b"<svg/>");
    }

    #[test]
    fn a_root_covers_its_subtree_but_traversal_out_of_it_is_a_bad_request() {
        let tmp = tempfile::tempdir().unwrap();
        let inside = tmp.path().join("deep").join("clip.mp4");
        std::fs::create_dir_all(inside.parent().unwrap()).unwrap();
        std::fs::write(&inside, [0u8; 10]).unwrap();
        scope().allow_root(tmp.path());
        assert_eq!(get(&encoded(&inside), None).status(), StatusCode::OK);
        let escaped = tmp.path().join("deep").join("..").join("..").join("x.mp4");
        assert_eq!(
            get(&encoded(&escaped), None).status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            get("/relative/path.mp4", None).status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            get(&encoded(&tmp.path().join("missing.mp4")), None).status(),
            StatusCode::NOT_FOUND
        );
    }

    #[test]
    fn ranges_are_partial_content_with_the_headers_a_video_element_needs() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("a.wav");
        std::fs::write(&file, (0..100u8).collect::<Vec<_>>()).unwrap();
        scope().grant_file(&file);
        let r = get(&encoded(&file), Some("bytes=10-19"));
        assert_eq!(r.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(header_of(&r, header::CONTENT_RANGE), "bytes 10-19/100");
        assert_eq!(header_of(&r, header::CONTENT_LENGTH), "10");
        assert_eq!(r.body(), &(10..20u8).collect::<Vec<_>>());
        let open = get(&encoded(&file), Some("bytes=95-"));
        assert_eq!(header_of(&open, header::CONTENT_RANGE), "bytes 95-99/100");
        let suffix = get(&encoded(&file), Some("bytes=-5"));
        assert_eq!(header_of(&suffix, header::CONTENT_RANGE), "bytes 95-99/100");
        let past = get(&encoded(&file), Some("bytes=100-200"));
        assert_eq!(past.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        assert_eq!(header_of(&past, header::CONTENT_RANGE), "bytes */100");
        let clipped = get(&encoded(&file), Some("bytes=0-1000"));
        assert_eq!(header_of(&clipped, header::CONTENT_RANGE), "bytes 0-99/100");
    }

    #[test]
    fn every_answer_allows_the_webview_origin_and_a_preflight_is_answered_without_a_file() {
        let denied = get("/relative.mp4", None);
        assert_eq!(header_of(&denied, header::ACCESS_CONTROL_ALLOW_ORIGIN), "*");
        let preflight = handle(
            &Request::builder()
                .method(tauri::http::Method::OPTIONS)
                .uri("recast-asset://localhost/anything")
                .body(Vec::new())
                .unwrap(),
        );
        assert_eq!(preflight.status(), StatusCode::NO_CONTENT);
        assert!(header_of(&preflight, header::ACCESS_CONTROL_ALLOW_HEADERS).contains("range"));
        assert_eq!(
            header_of(&preflight, header::ACCESS_CONTROL_ALLOW_ORIGIN),
            "*"
        );
    }

    #[test]
    fn a_long_range_is_clipped_so_one_answer_never_holds_a_whole_recording() {
        assert_eq!(
            parse_range("bytes=0-", 10 * MAX_RANGE_BYTES),
            Some((0, MAX_RANGE_BYTES - 1))
        );
        assert_eq!(
            parse_range("bytes=0-1,5-6", 100),
            None,
            "multi-range is not honoured"
        );
        assert_eq!(parse_range("items=0-1", 100), None);
        assert_eq!(parse_range("bytes=5-2", 100), None);
    }

    #[test]
    fn document_named_files_are_granted_wherever_they_sit_in_the_state() {
        let tmp = tempfile::tempdir().unwrap();
        let music = tmp.path().join("song.mp3");
        std::fs::write(&music, b"id3").unwrap();
        let state = serde_json::json!({
            "musicClips": [{ "source": music.to_string_lossy() }],
            "backgroundValue": "#0f172a",
            "nested": { "deeper": [tmp.path().join("missing.png").to_string_lossy()] }
        });
        scope().grant_named_in(&state);
        assert!(scope().is_allowed(&music));
        assert!(!scope().is_allowed(&tmp.path().join("missing.png")));
    }
}
