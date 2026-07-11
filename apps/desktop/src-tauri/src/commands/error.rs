//! Typed error for the Tauri IPC boundary.
//!
//! Commands historically returned `Result<T, String>`, building the message ad
//! hoc with `format!("{e:#}")` / `.to_string()` / string literals. That gave the
//! frontend nothing to branch on — only a string to match. [`AppError`] replaces
//! those with a typed error that carries a stable machine [`AppError::code`],
//! while its `Serialize` impl stays a plain string so the existing frontend
//! `String(err)` sites keep working unchanged.
//!
//! Migration is mechanical: a command returns [`AppResult<T>`] and uses `?`
//! (an `anyhow::Error`/`io::Error` converts automatically) or
//! [`AppError::msg`] for a contextual message. When the whole command layer is
//! migrated, flipping the `Serialize` impl to a structured `{code, message}`
//! object (plus a frontend invoke wrapper) unlocks per-kind error handling in
//! one coordinated change — every error already flows through this one type.

use serde::{Serialize, Serializer};

/// An error crossing the Tauri IPC boundary.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// A message-only error — string literals and contextual `format!`s that
    /// don't (yet) have a more specific kind.
    #[error("{0}")]
    Message(String),
    /// Wraps an [`anyhow`] chain. Display uses the alternate `{:#}` form so the
    /// full `.context()` chain is preserved (matching the old
    /// `format!("{e:#}")` sites).
    #[error("{0:#}")]
    Anyhow(#[from] anyhow::Error),
    /// A filesystem/IO error.
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

impl AppError {
    /// Build a message-only error from anything `Display`. Use for contextual
    /// messages, e.g. `AppError::msg(format!("… failed: {e:#}"))`.
    pub fn msg(message: impl std::fmt::Display) -> Self {
        Self::Message(message.to_string())
    }
}

// A bare string is by far the most common ad-hoc error today; accept it so
// `.ok_or_else(|| "…".to_string())?` and friends migrate with just `?`.
impl From<String> for AppError {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

impl From<&str> for AppError {
    fn from(message: &str) -> Self {
        Self::Message(message.to_string())
    }
}

impl Serialize for AppError {
    /// Serialize to a plain string — backward-compatible with the frontend's
    /// current `String(err)` handling. This is the single choke point to change
    /// when moving to a structured wire format.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

/// A `Result` whose error crosses the IPC boundary as an [`AppError`].
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_serializes_to_the_plain_string() {
        let json = serde_json::to_string(&AppError::from("boom")).unwrap();
        assert_eq!(json, "\"boom\"");
    }

    #[test]
    fn anyhow_display_preserves_the_full_context_chain() {
        let err: AppError = anyhow::anyhow!("root cause")
            .context("outer context")
            .into();
        // `{:#}` joins the chain with `: ` — the same text the old
        // `format!("{e:#}")` sites produced.
        assert_eq!(err.to_string(), "outer context: root cause");
    }

    #[test]
    fn io_error_serializes_to_its_message() {
        let err: AppError = std::io::Error::new(std::io::ErrorKind::NotFound, "nope").into();
        assert_eq!(serde_json::to_string(&err).unwrap(), "\"nope\"");
    }
}
