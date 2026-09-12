//! A windowed view over a long homogeneous track, guarded so a clipped or summarised copy is detectable.
//! `n` and `span` describe the FULL projected track; `rows` is only the window. Column one is always the monotonic key.

use serde::Serialize;
use serde_json::Value;

/// Half-open output-axis window `[start, end)` in seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Window {
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum WindowError {
    #[error("window end {end} is not after its start {start}")]
    Empty { start: f64, end: f64 },
    #[error("window bounds must be finite seconds")]
    NotFinite,
}

impl Window {
    /// # Errors On a non-finite or empty window.
    pub fn new(start: f64, end: f64) -> Result<Self, WindowError> {
        if !start.is_finite() || !end.is_finite() {
            return Err(WindowError::NotFinite);
        }
        if end <= start {
            return Err(WindowError::Empty { start, end });
        }
        Ok(Self { start, end })
    }

    pub fn contains(&self, t: f64) -> bool {
        t >= self.start && t < self.end
    }
}

/// One row: the key first, then the columns in `cols` order.
pub type Row = Vec<Value>;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrackView {
    pub kind: &'static str,
    /// Always `output`: every key is output-axis seconds so the agent never converts.
    pub axis: &'static str,
    pub cols: Vec<&'static str>,
    /// Rows in the full track, not in this window.
    pub n: usize,
    /// First key and last end of the full track, or `None` for an empty track.
    pub span: Option<(f64, f64)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<(f64, f64)>,
    pub rows: Vec<Row>,
    /// A one-line statement of what the reader should know when `rows` is empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl TrackView {
    /// Builds the view from every projected row, keeping only those whose key falls in `window`. Rows must arrive sorted by key.
    pub fn build(
        kind: &'static str,
        cols: Vec<&'static str>,
        rows: Vec<(f64, f64, Row)>,
        window: Option<Window>,
    ) -> Self {
        let n = rows.len();
        let span = match (rows.first(), rows.last()) {
            (Some(first), Some(last)) => Some((first.0, last.1)),
            _ => None,
        };
        let kept: Vec<Row> = rows
            .into_iter()
            .filter(|(key, _, _)| window.is_none_or(|w| w.contains(*key)))
            .map(|(_, _, row)| row)
            .collect();
        let note = match (n, kept.is_empty(), window) {
            (0, _, _) => Some(format!("the project has no {kind} rows")),
            (_, true, Some(w)) => Some(format!(
                "no {kind} rows start inside {:.3}..{:.3}; the track spans {:.3}..{:.3}",
                w.start,
                w.end,
                span.map_or(0.0, |s| s.0),
                span.map_or(0.0, |s| s.1)
            )),
            _ => None,
        };
        Self {
            kind,
            axis: "output",
            cols,
            n,
            span,
            window: window.map(|w| (w.start, w.end)),
            rows: kept,
            note,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rows() -> Vec<(f64, f64, Row)> {
        vec![
            (1.0, 1.4, vec![json!(1.0), json!(0.4), json!("a")]),
            (2.0, 2.3, vec![json!(2.0), json!(0.3), json!("b")]),
            (3.0, 3.9, vec![json!(3.0), json!(0.9), json!("c")]),
        ]
    }

    #[test]
    fn a_window_keeps_only_rows_whose_key_starts_inside_it() {
        let view = TrackView::build(
            "words",
            vec!["t", "d", "w"],
            rows(),
            Some(Window::new(1.5, 3.0).unwrap()),
        );
        assert_eq!(view.rows.len(), 1);
        assert_eq!(view.rows[0][2], json!("b"));
        assert_eq!(view.n, 3);
        assert_eq!(view.span, Some((1.0, 3.9)));
        assert_eq!(view.window, Some((1.5, 3.0)));
        assert!(view.note.is_none());
    }

    #[test]
    fn no_window_returns_every_row_and_no_window_field() {
        let view = TrackView::build("words", vec!["t", "d", "w"], rows(), None);
        assert_eq!(view.rows.len(), 3);
        assert!(view.window.is_none());
        assert!(!serde_json::to_string(&view).unwrap().contains("window"));
    }

    #[test]
    fn an_empty_window_says_where_the_track_actually_is() {
        let view = TrackView::build(
            "words",
            vec!["t"],
            rows(),
            Some(Window::new(10.0, 12.0).unwrap()),
        );
        assert!(view.rows.is_empty());
        assert_eq!(view.n, 3);
        assert!(view.note.unwrap().contains("1.000..3.900"));
    }

    #[test]
    fn an_empty_track_says_so_instead_of_returning_nothing() {
        let view = TrackView::build("silences", vec!["t"], Vec::new(), None);
        assert_eq!(view.n, 0);
        assert_eq!(view.span, None);
        assert_eq!(
            view.note.as_deref(),
            Some("the project has no silences rows")
        );
    }

    #[test]
    fn a_backwards_or_non_finite_window_is_refused() {
        assert_eq!(
            Window::new(3.0, 3.0).unwrap_err(),
            WindowError::Empty {
                start: 3.0,
                end: 3.0
            }
        );
        assert_eq!(
            Window::new(f64::NAN, 1.0).unwrap_err(),
            WindowError::NotFinite
        );
    }
}
