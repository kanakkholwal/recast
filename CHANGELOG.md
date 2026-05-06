# Changelog

All notable changes to Recast are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The in-app source of truth is [`apps/desktop/src/constants/changelog.ts`](apps/desktop/src/constants/changelog.ts).
Keep this file in sync with that module on every release.

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
