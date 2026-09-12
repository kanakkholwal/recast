//! Deny-first checks every agent-facing verb runs before touching a project.

use std::path::{Component, Path, PathBuf};

use serde_json::{json, Value};

/// Ops one append may carry. Larger batches are refused with the count rather than split, so a partial landing is impossible.
pub const MAX_OPS_PER_APPEND: usize = 200;

/// Characters a tool result may carry before its largest array is cut. About 15k tokens, well under the 25k most clients cap at.
pub const RESPONSE_BUDGET_CHARS: usize = 60_000;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum GuardError {
    #[error("'{0}' is not a .recast project; pass the path recast_project_list printed")]
    NotAProject(String),
    #[error("'{0}' does not exist")]
    Missing(String),
    #[error("'{0}' walks outside its directory; pass an absolute path")]
    Traversal(String),
    #[error("{count} ops in one append; the cap is {MAX_OPS_PER_APPEND}, send the rest in a second append")]
    TooManyOps { count: usize },
}

/// A project path an agent handed us, proven to name an existing `.recast` before any verb reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectPath(PathBuf);

impl ProjectPath {
    /// # Errors On a path without the `.recast` extension, with `..` in it, or that does not exist.
    pub fn parse(raw: &str) -> Result<Self, GuardError> {
        let path = Path::new(raw);
        if path.components().any(|c| matches!(c, Component::ParentDir)) {
            return Err(GuardError::Traversal(raw.to_string()));
        }
        if !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("recast"))
        {
            return Err(GuardError::NotAProject(raw.to_string()));
        }
        if !path.exists() {
            return Err(GuardError::Missing(raw.to_string()));
        }
        Ok(Self(path.to_path_buf()))
    }

    pub fn as_str(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}

/// # Errors When `count` exceeds [`MAX_OPS_PER_APPEND`].
pub fn check_batch_size(count: usize) -> Result<(), GuardError> {
    if count > MAX_OPS_PER_APPEND {
        return Err(GuardError::TooManyOps { count });
    }
    Ok(())
}

/// Cuts the largest array in `value` until the whole result fits the budget, recording what was dropped and how to ask for less.
/// Cutting an array rather than the tail of the text keeps the result valid JSON the model can still parse.
pub fn within_budget(value: Value, hint: &str) -> Value {
    within(value, hint, RESPONSE_BUDGET_CHARS)
}

fn within(mut value: Value, hint: &str, budget: usize) -> Value {
    let size = |v: &Value| v.to_string().len();
    if size(&value) <= budget {
        return value;
    }
    let Some(key) = largest_array_key(&value) else {
        return value;
    };
    let total = value[&key].as_array().map_or(0, Vec::len);
    let mut kept = total;
    while kept > 0 && size(&value) > budget {
        kept /= 2;
        if let Some(items) = value[&key].as_array_mut() {
            items.truncate(kept);
        }
    }
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "truncated".into(),
            json!({ "field": key, "kept": kept, "total": total, "hint": hint }),
        );
    }
    value
}

fn largest_array_key(value: &Value) -> Option<String> {
    value
        .as_object()?
        .iter()
        .filter_map(|(k, v)| v.as_array().map(|a| (k, a.len())))
        .max_by_key(|(_, len)| *len)
        .map(|(k, _)| k.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_without_the_extension_is_refused_with_the_next_step() {
        let err = ProjectPath::parse("C:/videos/take.mp4").unwrap_err();
        assert!(matches!(err, GuardError::NotAProject(_)));
        assert!(err.to_string().contains("recast_project_list"));
    }

    #[test]
    fn a_traversing_path_is_refused_before_the_filesystem_is_asked() {
        let err = ProjectPath::parse("../secrets/x.recast").unwrap_err();
        assert_eq!(err, GuardError::Traversal("../secrets/x.recast".into()));
    }

    #[test]
    fn a_missing_project_is_refused() {
        let dir = std::env::temp_dir().join("recast-guard-missing.recast");
        let _ = std::fs::remove_file(&dir);
        let err = ProjectPath::parse(&dir.to_string_lossy()).unwrap_err();
        assert!(matches!(err, GuardError::Missing(_)));
    }

    #[test]
    fn an_existing_project_passes_and_keeps_its_path() {
        let file = std::env::temp_dir().join("recast-guard-exists.recast");
        std::fs::write(&file, b"").unwrap();
        let parsed = ProjectPath::parse(&file.to_string_lossy()).unwrap();
        assert_eq!(parsed.as_str(), file.to_string_lossy());
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn the_batch_cap_names_the_count() {
        let err = check_batch_size(MAX_OPS_PER_APPEND + 1).unwrap_err();
        assert_eq!(
            err,
            GuardError::TooManyOps {
                count: MAX_OPS_PER_APPEND + 1
            }
        );
        assert!(check_batch_size(MAX_OPS_PER_APPEND).is_ok());
    }

    #[test]
    fn a_small_result_is_returned_untouched() {
        let value = json!({ "rows": [1, 2, 3] });
        assert_eq!(within(value.clone(), "narrow", 1000), value);
    }

    #[test]
    fn an_oversized_result_is_cut_on_its_largest_array_and_says_so() {
        let value = json!({ "small": [1], "rows": (0..500).collect::<Vec<_>>() });
        let cut = within(value, "pass a window", 400);
        assert!(cut.to_string().len() <= 400);
        let note = &cut["truncated"];
        assert_eq!(note["field"], "rows");
        assert_eq!(note["total"], 500);
        assert!(note["kept"].as_u64().unwrap() < 500);
        assert_eq!(note["hint"], "pass a window");
        assert_eq!(cut["small"], json!([1]));
    }

    #[test]
    fn a_result_with_no_array_is_left_alone_even_when_large() {
        let value = json!({ "text": "x".repeat(2000) });
        assert_eq!(within(value.clone(), "", 100), value);
    }
}
