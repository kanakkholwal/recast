//! Inhibit display + system sleep while a long-running capture or export is
//! active — a recording that dies because the display slept, or an export that
//! stalls when the machine idles, is a trust-breaking bug.
//!
//! A single dedicated OS thread owns the platform inhibitor so its guard never
//! has to be `Send`/`Sync` (the Linux backend holds a D-Bus connection that
//! isn't). Commands talk to it over a channel, and it ref-counts holders so a
//! recording and an export can each keep the machine awake independently — the
//! inhibitor lifts only when the last holder releases.

use std::sync::mpsc::{channel, Sender};
use std::thread;

enum Msg {
    Acquire,
    Release,
}

/// Handle stored in `AppState`. Cheap and `Send + Sync` (it's just a channel
/// sender); the real inhibitor lives on the worker thread.
pub struct PowerManager {
    tx: Sender<Msg>,
}

impl PowerManager {
    pub fn new() -> Self {
        let (tx, rx) = channel::<Msg>();
        thread::Builder::new()
            .name("recast-power".into())
            .spawn(move || {
                let mut holders: u32 = 0;
                // `keepawake::KeepAwake` lives only here; dropping it releases
                // the OS inhibitor. Never crosses the thread boundary.
                let mut guard: Option<keepawake::KeepAwake> = None;
                while let Ok(msg) = rx.recv() {
                    match msg {
                        Msg::Acquire => {
                            holders += 1;
                            if guard.is_none() {
                                match keepawake::Builder::default()
                                    .display(true)
                                    .idle(true)
                                    .sleep(true)
                                    .reason("Recast is recording or exporting")
                                    .app_name("Recast")
                                    .app_reverse_domain("com.nexonauts.recast")
                                    .create()
                                {
                                    Ok(g) => guard = Some(g),
                                    Err(e) => log::warn!("keep-awake acquire failed: {e}"),
                                }
                            }
                        }
                        Msg::Release => {
                            holders = holders.saturating_sub(1);
                            if holders == 0 {
                                guard = None;
                            }
                        }
                    }
                }
            })
            .expect("spawn recast-power thread");
        Self { tx }
    }

    /// Acquire a hold whose release is a separate call — used by recording,
    /// whose awake-lifetime spans two commands (`start_recording` →
    /// `stop_recording`). Pair every `acquire` with exactly one `release`.
    pub fn acquire(&self) {
        let _ = self.tx.send(Msg::Acquire);
    }

    pub fn release(&self) {
        let _ = self.tx.send(Msg::Release);
    }

    /// RAII hold for a single scope (e.g. one `export_video` call). Releases on
    /// drop — covering early returns, `?` errors, and unwinds.
    pub fn lease(&self) -> PowerLease {
        let _ = self.tx.send(Msg::Acquire);
        PowerLease {
            tx: self.tx.clone(),
        }
    }
}

/// Drop-guard returned by [`PowerManager::lease`].
pub struct PowerLease {
    tx: Sender<Msg>,
}

impl Drop for PowerLease {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Release);
    }
}
