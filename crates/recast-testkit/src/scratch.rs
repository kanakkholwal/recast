use std::path::{Path, PathBuf};

/// A temporary directory that removes itself, including after a failing test.
/// Replaces hand-rolled pid-keyed dirs, which leaked a set on every run.
#[derive(Debug)]
pub struct Scratch {
    path: PathBuf,
}

impl Scratch {
    /// A fresh directory for `label`, cleared if one was left behind. Panics if
    /// it cannot be created, which means the test could not have run anyway.
    #[must_use]
    pub fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "recast-{label}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("a scratch directory");
        Self { path }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// A path inside the scratch directory. The file need not exist.
    #[must_use]
    pub fn file(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

/// So a helper that used to return a `PathBuf` can return the guard instead
/// without every call site changing: `scratch.join("x")` still works.
impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<Path> for Scratch {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // Best effort: panicking during unwind would mask the real failure.
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scratch_directory_exists_while_it_is_held() {
        let scratch = Scratch::new("exists");
        assert!(scratch.path().is_dir());
    }

    #[test]
    fn dropping_it_removes_the_directory_and_everything_in_it() {
        let kept;
        {
            let scratch = Scratch::new("removed");
            std::fs::write(scratch.file("a.txt"), b"x").expect("write");
            kept = scratch.path().to_path_buf();
            assert!(kept.join("a.txt").exists());
        }
        assert!(!kept.exists(), "the directory outlived its guard");
    }

    /// The reason `Deref` is here: a helper can hand back the guard and callers
    /// keep using it as a path.
    #[test]
    fn a_guard_is_usable_wherever_its_path_was() {
        let scratch = Scratch::new("deref");
        let inside: &Path = &scratch;
        assert_eq!(inside, scratch.path());
        assert_eq!(scratch.join("a.txt"), scratch.file("a.txt"));
    }

    /// Two tests in one binary run on different threads; naming both after the
    /// pid alone would have them delete each other's files mid-run.
    #[test]
    fn two_guards_in_one_process_do_not_share_a_directory() {
        let a = Scratch::new("shared");
        let b = std::thread::spawn(|| Scratch::new("shared").path().to_path_buf())
            .join()
            .expect("thread");
        assert_ne!(a.path(), b.as_path());
    }
}
