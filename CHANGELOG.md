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

### Added
- Linux screen capture: a Wayland-native pipeline using `xdg-desktop-portal`
  + PipeWire, and a parallel X11 native capture path. Linux recording docs
  refreshed alongside the new backends.
- Recording profiles: per-project capture profiles with dynamic capability
  combinations, device awareness, and a profile-management UI in the
  desktop app.
- Camera overlay in the editor: composite the recorded camera track over
  the screen video with position presets, size, shape, and mirror toggles.
  Gated behind a `CAMERA_OVERLAY_UI_ENABLED` feature flag.
- Cursor: mouse-press events feed into the recorded timeline and a refreshed
  set of cursor styles ships with the editor.
- Native macOS-style page transitions via the View Transitions API, with a
  smoother titlebar handoff.
- Web download page redesigned with new icons and a features grid.

### Changed
- Command palette (⌘K) extracted into a global `CommandPaletteHost` mounted
  at the root layout, so the shortcut and dialog work on every route —
  including the editor — not only on routes that render the sidebar.
- Properties panel: shared `PanelSection` primitive replaces ~30 ad-hoc
  section headers, drops repeated panel-name titles, normalises gap to
  `gap-4`, and standardises toggle / reset placement across Background,
  Focus, Annotations, Cursor, Audio, Camera, and Info panels.
- Design tokens: introduced a Framer-inspired vocabulary (`canvas`,
  `surface-1/2`, `ink`, `ink-muted`, `hairline`, gradient spotlight cards,
  elevation shadows) layered on top of the existing shadcn tokens. Primary
  color and font stack preserved; legacy surface tokens stayed on their
  original values.

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
