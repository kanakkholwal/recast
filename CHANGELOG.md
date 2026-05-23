# Changelog

All notable changes to Recast are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This file is the **canonical source for both** the GitHub release notes and
the in-app "What's new" panel:

- **Releases** — `Release Desktop App` workflow runs
  `scripts/extract-changelog.mjs <tag>` and uses the matching
  `## [<version>]` section as the release body.
- **Desktop in-app** —
  [`apps/desktop/src/constants/changelog.ts`](apps/desktop/src/constants/changelog.ts)
  is **regenerated** from this file by `pnpm sync:changelog` (and
  automatically before each `pnpm dev` / `pnpm build` of the desktop app).
  Don't edit the `RELEASES` array directly — it lives between
  `RELEASES:START` / `RELEASES:END` markers and will be overwritten.
- **Web** — `apps/web/src/routes/changelog/+page.ts` reads from the
  GitHub Releases REST API at runtime, which means the same curated section
  surfaces there too as soon as the release publishes.

Headings must follow the literal form `## [<version>] — <date>` (em-dash) so
both the extractor and the desktop sync can find them. Subsections use
`### Added`, `### Changed`, `### Fixed`, `### Deprecated`. An optional
`### Highlights` block above those is rendered as the "punchy" bullet row in
the desktop dialog.

## Authoring entries

Add a changeset per PR instead of editing this file by hand for in-flight
work:

```sh
pnpm changeset
```

See [`.changeset/README.md`](.changeset/README.md) for the full flow.
`pnpm release:prepare <version>` consumes pending changesets and the current
`[Unreleased]` block into a new dated section.

## [Unreleased]

## [0.1.9] — 2026-05-23

### Added
- Inline playback for recordings: tapping a card on the exports page now
  opens a `PlayerDialog` powered by `@recast/player` (RecastPlayer) with the
  branded media-chrome controls, instead of jumping straight to the file
  location. "Show in folder" stays one click away inside the dialog footer.
- Global `@recast/player/styles.css` import in the desktop root layout so
  any future inline players pick up the same theming without per-route
  boilerplate.

### Fixed
- Pointer-events leak from floating UI surfaces in the Tauri build:
  `DropdownMenu`, `HoverCard`, `Popover`, and `Select` content wrappers now
  also default `preventScroll={false}` (matching the earlier `Dialog` and
  `Sheet` fix from 0.1.6), so a closed menu or popover can no longer leave
  `pointer-events: none` on the document body and freeze the window.

## [0.1.8] — 2026-05-22

### Added
- Pause and resume during recording with controls in the recording panel and
  a clearer status indicator, so a notification or knock at the door no longer
  forces a restart.
- Auto-updater and "What's new" notifications in the bottom-right corner of
  the editor, so release prompts and changelog nudges stay out of the way of
  the timeline.
- Silence detection (phase 1, opt-in under Settings → Experimental): finds
  dead-air segments by combining waveform analysis with cursor idleness, then
  offers one-click cuts you can review or dismiss.
- Dashboard route with a local-storage-backed data layer for recordings and
  exports, plus first analytics hooks.
- Web auth foundation: magic-link sign-in and password-reset flows backed by
  Better Auth + Drizzle, plus a public waitlist endpoint for Recast Cloud.
- macOS and Linux platform modules for audio and camera capture, paving the
  way for full feature parity with the Windows build.
- Homebrew Cask publishing workflow and matching install instructions for
  macOS alongside the existing `.dmg`, `.deb`, `.AppImage`, and `.exe`
  artifacts.

### Changed
- Smart-zoom suggestions: new scoring model that clusters clicks, weighs
  dwell time, and dedupes same-spot triggers, so auto-applied focus regions
  land on the moments that actually matter instead of every mouse-down.
- Toaster restyled to share visual language with the bottom-right update
  notifications: same card geometry, same close affordance, same icon-badge
  variants. Sits in `bottom-right` everywhere instead of `top-center`.
- Marketing site: hero copy rewritten to honestly describe the timeline
  ("the lightest editor you've used") instead of pretending it doesn't
  exist; new editor-tour rail showcases the auto and manual tools side by
  side. Features, gamers, pricing pages refreshed too.
- Recordings library cards (web + desktop) picked up techy framing —
  dot-grid placeholders, primary glow, CRT-style corner brackets — so an
  empty thumbnail reads as "ready for a frame" instead of an empty hole.

### Fixed
- Window-freeze regression on recording start: every FFmpeg/ffprobe spawn
  site now uses `configure_silent_command` on Windows so the console flash
  no longer steals focus and reads as "the whole window is frozen".
- Closing the recorder window while a recording is in flight no longer
  drops the capture; the app prompts and resolves the save first.

## [0.1.7] — 2026-05-16

### Added
- Bulk-select mode for recordings and exports, with a floating action bar
  for delete and a single-tap "select all".
- Morph animations when toggling between grid and list views on the
  recordings and exports pages — same items, no jarring re-flow.
- One-shot setup scripts (`setup.ps1` / `setup.sh`) so first-time
  contributors can bring the whole monorepo up with a single command on
  Windows or macOS/Linux.

### Changed
- Export filenames now suffix duplicates with `(1)`, `(2)`, ... via a
  shared `unique_path` helper, so re-exporting the same recording keeps
  both files instead of silently overwriting the previous one.
- Quick-start docs screenshot refreshed to show region selection.

### Fixed
- Hero CTA region: removed an unused background layer that was painting a
  stray gradient behind the headline on some viewport widths.

## [0.1.6] — 2026-05-10

### Added
- Version-sync release scripts: every build manifest validates against the
  release tag and fails fast if a `0.0.0-dev` placeholder slips through.
- GitHub issue templates for bug reports, feature requests, and
  performance issues.

### Changed
- Dialog and Sheet components default `preventScroll={false}` so a closed
  dialog can no longer leak `pointer-events: none` onto the document body
  inside Tauri — the root cause of the earlier "the whole window is dead"
  reports.

### Fixed
- Resolved an intermittent pointer-blockage bug in the Dialog component
  that froze interactions after closing a modal.
- Version placeholders unified across files so dev and release builds no
  longer disagree about who they are.

## [0.1.5] — 2026-05-09

### Added
- Linux screen capture: a Wayland-native pipeline using
  `xdg-desktop-portal` + PipeWire, and a parallel X11 native capture
  path. Linux recording docs refreshed alongside the new backends.
- Recording profiles: per-launch capture profiles with dynamic capability
  combinations, device awareness, and a management UI in Settings.
- Command palette (⌘K) extracted into a global `CommandPaletteHost`
  mounted at the root layout, so the shortcut and dialog work on every
  route — including the editor — not only on routes that render the
  sidebar.
- Web download page redesigned with new platform icons and a feature
  grid.

### Changed
- Properties panel: shared `PanelSection` primitive replaces ~30 ad-hoc
  section headers, drops repeated panel-name titles, normalises gap to
  `gap-4`, and standardises toggle / reset placement across Background,
  Focus, Annotations, Cursor, Audio, Camera, and Info panels.
- Design tokens: introduced a Framer-inspired vocabulary (`canvas`,
  `surface-1/2`, `ink`, `ink-muted`, `hairline`, gradient spotlight cards,
  elevation shadows) layered on top of the existing shadcn tokens.
  Primary colour and font stack preserved.

## [0.1.4] — 2026-05-08

### Added
- Camera overlay in the editor: composite the recorded camera track over
  the screen video with position presets, size, shape, and mirror
  toggles. Gated behind a `CAMERA_OVERLAY_UI_ENABLED` feature flag.
- Cursor: mouse-press events feed into the recorded timeline, and a
  refreshed set of cursor styles ships with the editor.
- Native macOS-style page transitions via the View Transitions API, with
  a smoother titlebar handoff between routes.

### Changed
- Canvas geometry and aspect-ratio handling: editor geometry helpers now
  carry the chosen aspect end-to-end (preview, composite, drop-shadow)
  without per-call ad-hoc math.

## [0.1.3-beta] — 2026-05-07

### Added
- Active-preset chip in the editor toolbar with a reset-to-source affordance.
- Per-project preset persistence: applied preset and output aspect round-trip
  with undo/redo and project autosave.

### Changed
- GIF export now uses a 2-pass palettegen → paletteuse pipeline, so the
  progress bar advances in real time instead of sitting at 0% while only the
  elapsed counter ticked.
- Presets actually resize the canvas to their target aspect (16:9, 9:16,
  1:1, 1.91:1) end-to-end through the preview, FFmpeg filter graph, cursor
  overlay, and drop-shadow rasteriser.
- Stronger blur annotation: redacts content even at full strength, with
  scaled tint opacity and an optional gray wash above 0.6 strength.
- FFmpeg error reporting filters out progress noise so real diagnostic
  lines reach the failure toast.

### Fixed
- Region picker "Use area" / "Cancel" buttons now work; closing the main
  window exits the app instead of leaving aux windows holding the process.
- Quick action no longer opens the camera preview inside the recording
  panel window.

## [0.1.2-beta] — 2026-05-06

### Added
- Timeline workspace: clip bar, playhead, ruler, toolbar, and zoom lane components.
- Blur annotations with adjustable strength, rendered through the composite canvas pipeline.
- Cursor animation effects: click bounce, idle sway, and motion blur.
- Glass card and chip components for a more refined UI surface.
- `Kbd` component for consistent keyboard shortcut hints.
- Region selection in the source picker, with last-used source persistence.
- Camera overlay settings and validation, plus browser-based camera enumeration.
- Command palette (⌘K) with global navigation, recording, theme and external commands.
- Sidebar pinning and hover behavior.

### Changed
- Refactored project structure for readability and maintainability.
- Upgraded Node.js to v24 and enabled `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24`.
- Redesigned loading screen with new logo and progress bar.
- Polished typography, spacing, and accessibility across annotation panels and headers.

### Fixed
- Reverted erroneous app version bump; settings layout regressions cleaned up.

## [0.1.0-beta] — Initial beta

- First public beta of Recast: offline-first desktop screen recorder and editor
  built on Tauri v2, Svelte 5, and Rust.
