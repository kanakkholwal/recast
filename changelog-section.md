### Highlights
- Scene animations: give any clip an entrance and exit — fade, slide, scale, pop, shrink, or rotate — that plays in the preview and renders in the exported video.
- Exports are roughly 3.5× faster. A 46-second recording that took 5m42s now finishes in about 1m37s.

### Added
- Scene animations. Each clip can animate into and out of view — fade, slide, scale, pop, shrink, or rotate — with full easing control per side. A project-wide motion tone (Subtle, Balanced, Energetic) tunes the intensity across the whole timeline, and a Push transition can carry motion across a cut where content was removed. Animations play in the preview and render in the exported video.

### Changed
- Export defaults to 60fps for recordings above 60fps (Original, 30, and 24 stay selectable). It's imperceptible for a screen recording and roughly halves export time.
- The background image is blurred once at export instead of on every frame, which more than halved the encode time on its own.
- The export dialog names each prep step, rendering the cursor and annotation layer and then encoding, so it never sits on a blank "Preparing…".

### Fixed
- Scene animations now render in exported video, not just the preview.
- Export progress and the time-remaining estimate are measured against the real output length, so the bar no longer stalls short of or overshoots 100% on projects with cuts or speed changes.
