//! On-demand Google Fonts: fetch a family's woff2 once and cache it on device,
//! so captions (and later annotations) can render any Google font offline after
//! the first use. Returns a local path the frontend loads via the FontFace API.
//!
//! The export burn-in needs a different format: libass/FreeType can't read
//! woff2, so [`ensure_caption_font_dir`] fetches the TTF (Google serves it to an
//! older UA) into a dedicated directory that the `ass` filter's `fontsdir`
//! points at — that's how a preset's branded font actually appears in the MP4.

use std::path::PathBuf;

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

/// Ensure the TTF for `family` at `weight` is cached under `app_data/fonts/ttf/`
/// and return that DIRECTORY (for libass `fontsdir`). Uses an older UA so Google
/// serves TTF instead of woff2. Called from the export burn-in path.
pub(crate) async fn ensure_caption_font_dir(
    app: &AppHandle,
    family: &str,
    weight: u32,
) -> Result<PathBuf, String> {
    // A per-family dir keeps `fontsdir` tiny — libass scans everything in it, so
    // pointing at one shared dir with many fonts would slow font matching.
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
        return Ok(dir);
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
    Ok(dir)
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
        // A css2 body can carry several formats; the picker must match on `ext`,
        // not just take the first url().
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
