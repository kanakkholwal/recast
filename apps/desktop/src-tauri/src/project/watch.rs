//! The second writer: `project.rcx` edited on disk by an agent or a person. A change is read back as ops against the
//! last checkpoint and sequenced like any other batch; our own checkpoint echoes back as `Echo` and is ignored.

use std::path::Path;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::Mutex;
use recast_project::layout;
use recast_project::store::{Expect, FileChange, StoreError};
use recast_project::Position;

/// Editors write in bursts (temp file, rename, a second flush); one read after the burst is enough.
pub const DEBOUNCE: Duration = Duration::from_millis(50);

/// What the watcher tells the app about a file it could not take.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Invalid {
    pub path: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<Position>,
}

pub type InvalidListener = Box<dyn Fn(Invalid) + Send + Sync>;

/// The owner's hook: applies a batch of foreign ops and answers whether it landed.
pub type Apply =
    Arc<dyn Fn(&[recast_project::Op], Expect) -> Result<bool, StoreError> + Send + Sync>;

/// Keeps a project's document file under watch for as long as it is held. Dropping it stops the watch.
pub struct FileWatch {
    _watcher: RecommendedWatcher,
}

impl FileWatch {
    /// # Errors When the platform watcher cannot be created or the directory cannot be watched.
    pub fn start(
        root: &Path,
        read: Arc<dyn Fn() -> Result<FileChange, StoreError> + Send + Sync>,
        apply: Apply,
        on_invalid: Arc<Mutex<Option<InvalidListener>>>,
    ) -> notify::Result<Self> {
        let document = root.join(layout::DOCUMENT);
        let (tx, rx) = mpsc::channel::<()>();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                if let Ok(event) = event {
                    if event.paths.iter().any(|p| same_file(p, &document)) {
                        let _ = tx.send(());
                    }
                }
            })?;
        watcher.watch(root, RecursiveMode::NonRecursive)?;
        let path = root.to_path_buf();
        std::thread::Builder::new()
            .name("recast-file-watch".into())
            .spawn(move || pump(&rx, &path, &read, &apply, &on_invalid))
            .map_err(|e| notify::Error::generic(&e.to_string()))?;
        Ok(Self { _watcher: watcher })
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    a.file_name() == b.file_name()
}

/// Coalesces a burst of events into one read, then settles it.
fn pump(
    rx: &mpsc::Receiver<()>,
    root: &Path,
    read: &Arc<dyn Fn() -> Result<FileChange, StoreError> + Send + Sync>,
    apply: &Apply,
    on_invalid: &Arc<Mutex<Option<InvalidListener>>>,
) {
    while rx.recv().is_ok() {
        let deadline = Instant::now() + DEBOUNCE;
        while let Ok(()) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {}
        settle(root, read.as_ref(), apply, on_invalid);
    }
}

/// One read of the file: echo, foreign ops applied, or an invalid file reported. Never touches the disk.
pub fn settle(
    root: &Path,
    read: &dyn Fn() -> Result<FileChange, StoreError>,
    apply: &Apply,
    on_invalid: &Mutex<Option<InvalidListener>>,
) {
    let invalid = |message: String, at: Option<Position>| {
        log::warn!("{}: {message}", root.display());
        if let Some(listener) = on_invalid.lock().as_ref() {
            listener(Invalid {
                path: root.to_string_lossy().into_owned(),
                message,
                at,
            });
        }
    };
    match read() {
        Ok(FileChange::Echo) => {}
        Ok(FileChange::Ops(ops)) => match apply(&ops, Expect::default()) {
            Ok(true) => log::info!(
                "{}: {} op(s) taken from the file",
                root.display(),
                ops.len()
            ),
            Ok(false) => invalid(
                "the file changed while a batch was landing; edit it again".into(),
                None,
            ),
            Err(e) => invalid(format!("the file's edit could not be applied: {e}"), None),
        },
        Err(StoreError::Parse(e)) => invalid(e.to_string(), e.position()),
        Err(e) => invalid(e.to_string(), None),
    }
}
