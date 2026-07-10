//! Keep the display and system awake while recording or exporting.
//!
//! A dedicated thread owns the platform inhibitor (its guard isn't Send on the
//! Linux D-Bus backend) and ref-counts holders, so recording and export can
//! hold it independently. The inhibitor lifts only when the last holder frees.

use std::sync::mpsc::{channel, Sender};
use std::thread;

enum Msg {
    Acquire,
    Release,
}

/// Stored in `AppState`. Just a channel sender, so it stays `Send + Sync`; the
/// real inhibitor lives on the worker thread.
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
                // Never crosses the thread boundary; dropping it frees the OS inhibitor.
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

    /// Acquire a hold released by a later `release` call. Recording uses this
    /// because its awake window spans two commands (start then stop).
    pub fn acquire(&self) {
        let _ = self.tx.send(Msg::Acquire);
    }

    pub fn release(&self) {
        let _ = self.tx.send(Msg::Release);
    }

    /// RAII hold for one scope (e.g. a single `export_video` call). Releases on
    /// drop, covering early returns, `?` errors, and unwinds.
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
