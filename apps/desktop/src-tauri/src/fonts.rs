//! On-demand Google Fonts: fetch a family's woff2 once and cache it on device,
//! so captions (and later annotations) can render any Google font offline after
//! the first use. Returns a local path the frontend loads via the FontFace API.
//!
//! Rendering a caption needs a different format: neither libass/FreeType nor
//! the engine's rustybuzz can read woff2, so [`ensure_caption_font_file`] fetches
//! the TTF (Google serves it to an older UA). The burn-in points the `ass`
//! filter's `fontsdir` at its parent; the engine reads its bytes.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

use crate::commands::error::{AppError, AppResult};

/// Ensure the woff2 for `family` at `weight` is cached under
/// `app_data/fonts/`, downloading it from Google Fonts on first use. Returns the
/// local file path.
#[tauri::command]
pub async fn ensure_google_font(app: AppHandle, family: String, weight: u32) -> AppResult<String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::msg(format!("app_data_dir unavailable: {e}")))?
        .join("fonts");
    let safe: String = family
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let dest = dir.join(format!("{safe}-{weight}.woff2"));
    if dest.exists() {
        return Ok(dest.to_string_lossy().to_string());
    }

    // A modern browser UA makes Google Fonts serve woff2 (older UAs get ttf).
    let client = reqwest::Client::builder()
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        )
        .build()
        .map_err(|e| AppError::msg(format!("client: {e}")))?;

    let css_url = format!(
        "https://fonts.googleapis.com/css2?family={}:wght@{weight}&display=swap",
        family.replace(' ', "+")
    );
    let css = client
        .get(&css_url)
        .send()
        .await
        .map_err(|e| AppError::msg(format!("font css request: {e}")))?
        .error_for_status()
        .map_err(|e| AppError::msg(format!("font css http: {e}")))?
        .text()
        .await
        .map_err(|e| AppError::msg(format!("font css body: {e}")))?;

    let woff2 = extract_font_url(&css, ".woff2")
        .ok_or_else(|| AppError::msg(format!("no woff2 URL for '{family}' in Google Fonts CSS")))?;

    crate::transcription::download_file(&client, &woff2, None, &dest, |_, _| {}).await?;
    Ok(dest.to_string_lossy().to_string())
}

/// The cached TTF for `family` at `weight`, downloading it on first use. Uses an
/// older UA so Google serves TTF instead of woff2.
pub(crate) async fn ensure_caption_font_file(
    app: &AppHandle,
    family: &str,
    weight: u32,
) -> Result<PathBuf, String> {
    // A per-family dir keeps `fontsdir` tiny: libass scans everything in it, so one shared dir would slow matching.
    let safe: String = family
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir unavailable: {e}"))?
        .join("fonts")
        .join("ttf")
        .join(&safe);
    let dest = dir.join(format!("{safe}-{weight}.ttf"));
    if dest.exists() {
        return Ok(dest);
    }

    // Old UA → Google Fonts serves a TTF (FreeType-readable) instead of woff2.
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/4.0")
        .build()
        .map_err(|e| format!("client: {e}"))?;
    let css_url = format!(
        "https://fonts.googleapis.com/css2?family={}:wght@{weight}&display=swap",
        family.replace(' ', "+")
    );
    let css = client
        .get(&css_url)
        .send()
        .await
        .map_err(|e| format!("font css request: {e}"))?
        .error_for_status()
        .map_err(|e| format!("font css http: {e}"))?
        .text()
        .await
        .map_err(|e| format!("font css body: {e}"))?;
    let ttf = extract_font_url(&css, ".ttf")
        .ok_or_else(|| format!("no ttf URL for '{family}' in Google Fonts CSS"))?;
    crate::transcription::download_file(&client, &ttf, None, &dest, |_, _| {}).await?;
    Ok(dest)
}

/// The directory holding only this family's TTF, for libass `fontsdir`.
///
/// A per-family dir keeps the scan tiny: libass matches against everything in
/// there, so one shared dir with many fonts slows every burn-in down.
pub(crate) async fn ensure_caption_font_dir(
    app: &AppHandle,
    family: &str,
    weight: u32,
) -> Result<PathBuf, String> {
    let file = ensure_caption_font_file(app, family, weight).await?;
    file.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("cached font for '{family}' has no parent directory"))
}

/// The TTF path for a caption font, for a renderer that needs the BYTES rather
/// than a fontconfig directory. The engine's shaper cannot read woff2, which is
/// what [`ensure_google_font`] caches for the DOM.
#[tauri::command]
pub async fn caption_font_file(app: AppHandle, family: String, weight: u32) -> AppResult<String> {
    ensure_caption_font_file(&app, &family, weight)
        .await
        .map(|p| p.to_string_lossy().to_string())
        .map_err(AppError::msg)
}

/// Pull the first font URL with `ext` out of a Google Fonts `css2` response
/// (`src: url(https://fonts.gstatic.com/…) format(...)`).
fn extract_font_url(css: &str, ext: &str) -> Option<String> {
    for part in css.split("url(").skip(1) {
        let end = part.find(')')?;
        let raw = part[..end].trim_matches(|c| c == '"' || c == '\'');
        if raw.ends_with(ext) {
            return Some(raw.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::extract_font_url;

    #[test]
    fn extract_font_url_returns_first_matching_extension() {
        let css = "src: url(https://fonts.gstatic.com/s/a/v1/x.woff2) format('woff2');";
        assert_eq!(
            extract_font_url(css, ".woff2").as_deref(),
            Some("https://fonts.gstatic.com/s/a/v1/x.woff2")
        );
    }

    #[test]
    fn extract_font_url_skips_other_extensions_to_the_requested_one() {
        // A css2 body can carry several formats, so the picker must match on `ext` rather than take the first url().
        let css = "src: url(https://x/a.woff2) format('woff2'), \
                   url(https://x/a.ttf) format('truetype');";
        assert_eq!(
            extract_font_url(css, ".ttf").as_deref(),
            Some("https://x/a.ttf")
        );
    }

    #[test]
    fn extract_font_url_is_none_when_absent() {
        assert_eq!(extract_font_url("no urls here", ".woff2"), None);
        assert_eq!(
            extract_font_url("src: url(https://x/a.woff2);", ".ttf"),
            None
        );
    }

    #[test]
    fn extract_font_url_strips_surrounding_quotes() {
        let css = "src: url(\"https://x/a.ttf\") format('truetype');";
        assert_eq!(
            extract_font_url(css, ".ttf").as_deref(),
            Some("https://x/a.ttf")
        );
    }
}
