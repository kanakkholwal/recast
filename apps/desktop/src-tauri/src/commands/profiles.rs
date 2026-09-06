//! Backend-owned recording profiles in `recast_profiles.json`, so the panel, picker and CLI read one store.
//! They used to live in WebView `localStorage`, which left `recast profile list` with nothing to read.

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use super::error::{AppError, AppResult};
use super::types::{AppState, CaptureIntent};

/// Emitted with the new `ProfilesSnapshot` whenever the profile set changes.
pub const PROFILES_CHANGED_EVENT: &str = "recording-profiles:changed";

/// A saved capture preset. v2 schema (device identity fields over the v1
/// capability-only shape). The nullable-but-required fields serialize as
/// explicit `null` to match the frontend's on-disk shape; `countdown` is the
/// one optional field (absent = inherit the global countdown).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingProfile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub system_audio: bool,
    #[serde(default)]
    pub microphone: bool,
    /// Tauri/Rust audio device id; null = system default when applied.
    #[serde(default)]
    pub mic_device_id: Option<String>,
    /// Display label for the saved mic; fallback identity if the id goes stale.
    #[serde(default)]
    pub mic_label: Option<String>,
    #[serde(default)]
    pub camera: bool,
    /// DirectShow-friendly name: what the Rust recorder consumes.
    #[serde(default)]
    pub camera_label: Option<String>,
    /// Browser MediaDevices id: what the camera-preview window consumes.
    #[serde(default)]
    pub camera_device_id: Option<String>,
    /// Per-profile countdown override (seconds). Absent = inherit global; `0` = off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub countdown: Option<u32>,
    #[serde(default)]
    pub is_default: bool,
}

/// The persisted profile set: the list plus the profile-system on/off flag.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilesState {
    #[serde(default)]
    pub profiles: Vec<RecordingProfile>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

impl Default for ProfilesState {
    fn default() -> Self {
        Self {
            profiles: Vec::new(),
            enabled: true,
        }
    }
}

impl ProfilesState {
    /// The first-launch set: three profiles covering the common shapes. Mirrors
    /// the frontend `seedProfiles`. Used as the in-memory default before any GUI
    /// session has pushed the user's real set, so `recast profile list` has
    /// content even on a machine whose window was never opened.
    pub fn seeded() -> Self {
        Self {
            profiles: seed_profiles(),
            enabled: true,
        }
    }
}

/// Snapshot returned to the frontend/CLI. `initialized` is false when the store
/// is the ephemeral in-memory seed (no `recast_profiles.json` yet); the frontend
/// uses that to migrate its `localStorage` profiles into the backend once.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilesSnapshot {
    pub profiles: Vec<RecordingProfile>,
    pub enabled: bool,
    pub initialized: bool,
}

/// A fresh v4-style id. Uses `rand` (already a dep) rather than pulling in the
/// `uuid` crate; only uniqueness and stability matter, not the exact format.
fn new_id() -> String {
    use rand::Rng;
    let mut b: [u8; 16] = rand::thread_rng().gen();
    b[6] = (b[6] & 0x0f) | 0x40; // version 4
    b[8] = (b[8] & 0x3f) | 0x80; // variant 1
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14],
        b[15]
    )
}

fn seed_profiles() -> Vec<RecordingProfile> {
    let base = |name: &str, microphone: bool, camera: bool, is_default: bool| RecordingProfile {
        id: new_id(),
        name: name.to_string(),
        system_audio: true,
        microphone,
        mic_device_id: None,
        mic_label: None,
        camera,
        camera_label: None,
        camera_device_id: None,
        countdown: None,
        is_default,
    };
    vec![
        base("Screen only", false, false, true),
        base("Tutorial", true, false, false),
        base("Presentation", true, true, false),
    ]
}

fn profiles_path(app: &AppHandle) -> PathBuf {
    let dir = match app.path().app_data_dir() {
        Ok(dir) => dir,
        Err(e) => {
            log::warn!(
                "app_data_dir unavailable ({e}); profiles will not persist between sessions"
            );
            std::env::temp_dir()
        }
    };
    dir.join("recast_profiles.json")
}

/// Read the persisted profile set. Returns `(state, initialized)` where
/// `initialized` is true only when a real `recast_profiles.json` was read;
/// missing/corrupt files fall back to the in-memory seed with `initialized =
/// false`. Never throws. Mirrors `system::load_config`'s corrupt-file handling.
pub fn load_profiles_state(app: &AppHandle) -> (ProfilesState, bool) {
    let path = profiles_path(app);
    match std::fs::read_to_string(&path) {
        Ok(data) => match serde_json::from_str::<ProfilesState>(&data) {
            Ok(state) => (state, true),
            Err(e) => {
                log::warn!(
                    "profiles at {} unreadable ({e}); backing up to .bak and seeding",
                    path.display()
                );
                let _ = std::fs::rename(&path, path.with_extension("json.bak"));
                (ProfilesState::seeded(), false)
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (ProfilesState::seeded(), false),
        Err(e) => {
            log::warn!(
                "failed to read profiles at {} ({e}); seeding",
                path.display()
            );
            (ProfilesState::seeded(), false)
        }
    }
}

/// Atomically persist the profile set. Silently no-ops if storage is unavailable.
pub(crate) fn save_profiles_state(app: &AppHandle, state: &ProfilesState) {
    let path = profiles_path(app);
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            log::warn!("failed to create profiles dir {}: {e}", parent.display());
            return;
        }
    }
    let data = match serde_json::to_string_pretty(state) {
        Ok(data) => data,
        Err(e) => {
            log::error!("failed to serialize profiles: {e}");
            return;
        }
    };
    let tmp = path.with_extension("json.tmp");
    if let Err(e) = super::system::write_atomic(&tmp, &path, data.as_bytes()) {
        log::warn!("failed to persist profiles to {}: {e}", path.display());
        let _ = std::fs::remove_file(&tmp);
    }
}

/// Current profile set as a wire snapshot. Shared by the Tauri command and the
/// CLI control server so both read one source.
pub fn profiles_snapshot(app: &AppHandle) -> ProfilesSnapshot {
    let state = app.state::<AppState>();
    let guard = state.profiles.read();
    ProfilesSnapshot {
        profiles: guard.profiles.clone(),
        enabled: guard.enabled,
        initialized: state.profiles_initialized.load(Ordering::Relaxed),
    }
}

/// Map a profile onto the capture intent: capture toggles + device pointers +
/// countdown, plus the active-profile id. Does NOT touch the source (a profile
/// is capture settings, not a screen/window). The camera pointer uses the
/// DirectShow label the recorder consumes, falling back to the browser id.
pub fn apply_profile_to_intent(intent: &mut CaptureIntent, profile: &RecordingProfile) {
    intent.options.system_audio = profile.system_audio;
    intent.options.microphone = profile.microphone;
    intent.options.microphone_device_id = if profile.microphone {
        profile.mic_device_id.clone()
    } else {
        None
    };
    intent.options.camera = profile.camera;
    intent.options.camera_device_id = if profile.camera {
        profile
            .camera_label
            .clone()
            .or_else(|| profile.camera_device_id.clone())
    } else {
        None
    };
    intent.countdown = profile.countdown;
    intent.active_profile_id = Some(profile.id.clone());
}

/// Apply the profile matching `id` (by id, then case-insensitive name) to the capture intent and return the updated intent. Shared by the command and the CLI control server.
pub fn use_profile_by_id(app: &AppHandle, id: &str) -> Result<CaptureIntent, String> {
    let profile = {
        let state = app.state::<AppState>();
        let guard = state.profiles.read();
        guard
            .profiles
            .iter()
            .find(|p| p.id == id)
            .or_else(|| {
                guard
                    .profiles
                    .iter()
                    .find(|p| p.name.eq_ignore_ascii_case(id))
            })
            .cloned()
    };
    let Some(profile) = profile else {
        return Err(format!("no profile matching '{id}'"));
    };
    Ok(super::intent::update_intent(app, |i| {
        apply_profile_to_intent(i, &profile)
    }))
}

#[tauri::command]
pub fn get_profiles(app: AppHandle) -> ProfilesSnapshot {
    profiles_snapshot(&app)
}

#[tauri::command]
pub fn set_profiles(
    app: AppHandle,
    profiles: Vec<RecordingProfile>,
    enabled: bool,
) -> ProfilesSnapshot {
    let state = app.state::<AppState>();
    // Snapshot under the lock, drop it, then write: never hold the lock across a disk write, as the config store does.
    let to_save = {
        let mut guard = state.profiles.write();
        guard.profiles = profiles;
        guard.enabled = enabled;
        guard.clone()
    };
    state.profiles_initialized.store(true, Ordering::Relaxed);
    save_profiles_state(&app, &to_save);
    let snap = profiles_snapshot(&app);
    let _ = app.emit(PROFILES_CHANGED_EVENT, &snap);
    snap
}

#[tauri::command]
pub fn use_profile(app: AppHandle, id: String) -> AppResult<CaptureIntent> {
    use_profile_by_id(&app, &id).map_err(AppError::msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn profile(id: &str, name: &str) -> RecordingProfile {
        RecordingProfile {
            id: id.into(),
            name: name.into(),
            system_audio: true,
            microphone: false,
            mic_device_id: None,
            mic_label: None,
            camera: false,
            camera_label: None,
            camera_device_id: None,
            countdown: None,
            is_default: false,
        }
    }

    #[test]
    fn seed_has_three_and_exactly_one_default() {
        let seed = seed_profiles();
        assert_eq!(seed.len(), 3);
        assert_eq!(seed.iter().filter(|p| p.is_default).count(), 1);
        // Ids are distinct.
        assert_ne!(seed[0].id, seed[1].id);
        assert_ne!(seed[1].id, seed[2].id);
    }

    #[test]
    fn profile_serializes_camelcase_with_explicit_nulls() {
        let v = serde_json::to_value(profile("p1", "Screen only")).unwrap();
        assert_eq!(v["id"], json!("p1"));
        assert_eq!(v["systemAudio"], json!(true));
        // Nullable-required fields are present as null (matches the TS shape).
        assert_eq!(v["micDeviceId"], json!(null));
        assert_eq!(v["cameraLabel"], json!(null));
        assert_eq!(v["isDefault"], json!(false));
        // countdown is the one optional field: omitted when None.
        assert!(v.get("countdown").is_none());
    }

    #[test]
    fn profile_deserializes_with_missing_optional_fields() {
        // The frontend may omit undefined fields; serde(default) must fill them.
        let p: RecordingProfile =
            serde_json::from_value(json!({"id":"x","name":"n","systemAudio":true})).unwrap();
        assert!(!p.microphone);
        assert_eq!(p.mic_device_id, None);
        assert_eq!(p.countdown, None);
    }

    #[test]
    fn apply_maps_capture_fields_and_active_id() {
        let mut prof = profile("p2", "Tutorial");
        prof.microphone = true;
        prof.mic_device_id = Some("mic-7".into());
        prof.camera = true;
        prof.camera_label = Some("HD Webcam".into());
        prof.camera_device_id = Some("browser-cam-id".into());
        prof.countdown = Some(3);
        prof.system_audio = false;

        let mut intent = CaptureIntent::default();
        apply_profile_to_intent(&mut intent, &prof);

        assert!(!intent.options.system_audio);
        assert!(intent.options.microphone);
        assert_eq!(
            intent.options.microphone_device_id.as_deref(),
            Some("mic-7")
        );
        assert!(intent.options.camera);
        // Camera pointer prefers the DirectShow label the recorder consumes.
        assert_eq!(
            intent.options.camera_device_id.as_deref(),
            Some("HD Webcam")
        );
        assert_eq!(intent.countdown, Some(3));
        assert_eq!(intent.active_profile_id.as_deref(), Some("p2"));
    }

    #[test]
    fn apply_clears_device_pointers_when_capability_off() {
        let mut prof = profile("p3", "Silent");
        prof.microphone = false;
        prof.mic_device_id = Some("stale-mic".into());
        prof.camera = false;
        prof.camera_label = Some("stale-cam".into());

        let mut intent = CaptureIntent::default();
        apply_profile_to_intent(&mut intent, &prof);

        assert!(!intent.options.microphone);
        assert_eq!(intent.options.microphone_device_id, None);
        assert!(!intent.options.camera);
        assert_eq!(intent.options.camera_device_id, None);
    }

    #[test]
    fn snapshot_serializes_camelcase() {
        let snap = ProfilesSnapshot {
            profiles: vec![profile("a", "A")],
            enabled: false,
            initialized: true,
        };
        let v = serde_json::to_value(&snap).unwrap();
        assert_eq!(v["enabled"], json!(false));
        assert_eq!(v["initialized"], json!(true));
        assert_eq!(v["profiles"][0]["id"], json!("a"));
    }
}
