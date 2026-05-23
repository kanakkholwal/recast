import type { Component } from "svelte";
import { Bug, RefreshCw, Sparkles, Wrench } from "@lucide/svelte";

export type ChangeKind = "added" | "changed" | "fixed" | "deprecated";

export interface ChangelogChange {
	kind: ChangeKind;
	summary: string;
}

export interface ChangelogRelease {
	version: string;
	date: string;
	title?: string;
	highlights?: string[];
	changes: ChangelogChange[];
}

export const KIND_META: Record<
	ChangeKind,
	{ label: string; icon: Component<any>; tone: string }
> = {
	added: { label: "New", icon: Sparkles, tone: "text-primary" },
	changed: { label: "Changed", icon: RefreshCw, tone: "text-foreground" },
	fixed: { label: "Fixed", icon: Wrench, tone: "text-emerald-500" },
	deprecated: { label: "Deprecated", icon: Bug, tone: "text-amber-500" },
};

// Newest release first. The first entry's `version` is shown as the "latest".
// In-flight (unreleased) changes live only in CHANGELOG.md under
// `[Unreleased]`. They migrate here once the version is bumped and tagged.
//
// The block between RELEASES:START and RELEASES:END is regenerated from the
// root CHANGELOG.md by `pnpm sync:changelog` (and automatically before each
// desktop build via the `predev` / `prebuild` hook). Edit CHANGELOG.md, not
// this array.
// RELEASES:START — auto-generated, do not edit by hand
export const RELEASES: readonly ChangelogRelease[] = [
	{
		version: '0.1.9',
		date: '2026-05-23',
		changes: [
			{ kind: 'added', summary: 'Inline playback for recordings: tapping a card on the exports page now opens a `PlayerDialog` powered by `@recast/player` (RecastPlayer) with the branded media-chrome controls, instead of jumping straight to the file location. "Show in folder" stays one click away inside the dialog footer.' },
			{ kind: 'added', summary: 'Global `@recast/player/styles.css` import in the desktop root layout so any future inline players pick up the same theming without per-route boilerplate.' },
			{ kind: 'fixed', summary: 'Pointer-events leak from floating UI surfaces in the Tauri build: `DropdownMenu`, `HoverCard`, `Popover`, and `Select` content wrappers now also default `preventScroll={false}` (matching the earlier `Dialog` and `Sheet` fix from 0.1.6), so a closed menu or popover can no longer leave `pointer-events: none` on the document body and freeze the window.' },
		],
	},
	{
		version: '0.1.8',
		date: '2026-05-22',
		changes: [
			{ kind: 'added', summary: 'Pause and resume during recording with controls in the recording panel and a clearer status indicator, so a notification or knock at the door no longer forces a restart.' },
			{ kind: 'added', summary: 'Auto-updater and "What\'s new" notifications in the bottom-right corner of the editor, so release prompts and changelog nudges stay out of the way of the timeline.' },
			{ kind: 'added', summary: 'Silence detection (phase 1, opt-in under Settings → Experimental): finds dead-air segments by combining waveform analysis with cursor idleness, then offers one-click cuts you can review or dismiss.' },
			{ kind: 'added', summary: 'Dashboard route with a local-storage-backed data layer for recordings and exports, plus first analytics hooks.' },
			{ kind: 'added', summary: 'Web auth foundation: magic-link sign-in and password-reset flows backed by Better Auth + Drizzle, plus a public waitlist endpoint for Recast Cloud.' },
			{ kind: 'added', summary: 'macOS and Linux platform modules for audio and camera capture, paving the way for full feature parity with the Windows build.' },
			{ kind: 'added', summary: 'Homebrew Cask publishing workflow and matching install instructions for macOS alongside the existing `.dmg`, `.deb`, `.AppImage`, and `.exe` artifacts.' },
			{ kind: 'changed', summary: 'Smart-zoom suggestions: new scoring model that clusters clicks, weighs dwell time, and dedupes same-spot triggers, so auto-applied focus regions land on the moments that actually matter instead of every mouse-down.' },
			{ kind: 'changed', summary: 'Toaster restyled to share visual language with the bottom-right update notifications: same card geometry, same close affordance, same icon-badge variants. Sits in `bottom-right` everywhere instead of `top-center`.' },
			{ kind: 'changed', summary: 'Marketing site: hero copy rewritten to honestly describe the timeline ("the lightest editor you\'ve used") instead of pretending it doesn\'t exist; new editor-tour rail showcases the auto and manual tools side by side. Features, gamers, pricing pages refreshed too.' },
			{ kind: 'changed', summary: 'Recordings library cards (web + desktop) picked up techy framing — dot-grid placeholders, primary glow, CRT-style corner brackets — so an empty thumbnail reads as "ready for a frame" instead of an empty hole.' },
			{ kind: 'fixed', summary: 'Window-freeze regression on recording start: every FFmpeg/ffprobe spawn site now uses `configure_silent_command` on Windows so the console flash no longer steals focus and reads as "the whole window is frozen".' },
			{ kind: 'fixed', summary: 'Closing the recorder window while a recording is in flight no longer drops the capture; the app prompts and resolves the save first.' },
		],
	},
	{
		version: '0.1.7',
		date: '2026-05-16',
		changes: [
			{ kind: 'added', summary: 'Bulk-select mode for recordings and exports, with a floating action bar for delete and a single-tap "select all".' },
			{ kind: 'added', summary: 'Morph animations when toggling between grid and list views on the recordings and exports pages — same items, no jarring re-flow.' },
			{ kind: 'added', summary: 'One-shot setup scripts (`setup.ps1` / `setup.sh`) so first-time contributors can bring the whole monorepo up with a single command on Windows or macOS/Linux.' },
			{ kind: 'changed', summary: 'Export filenames now suffix duplicates with `(1)`, `(2)`, ... via a shared `unique_path` helper, so re-exporting the same recording keeps both files instead of silently overwriting the previous one.' },
			{ kind: 'changed', summary: 'Quick-start docs screenshot refreshed to show region selection.' },
			{ kind: 'fixed', summary: 'Hero CTA region: removed an unused background layer that was painting a stray gradient behind the headline on some viewport widths.' },
		],
	},
	{
		version: '0.1.6',
		date: '2026-05-10',
		changes: [
			{ kind: 'added', summary: 'Version-sync release scripts: every build manifest validates against the release tag and fails fast if a `0.0.0-dev` placeholder slips through.' },
			{ kind: 'added', summary: 'GitHub issue templates for bug reports, feature requests, and performance issues.' },
			{ kind: 'changed', summary: 'Dialog and Sheet components default `preventScroll={false}` so a closed dialog can no longer leak `pointer-events: none` onto the document body inside Tauri — the root cause of the earlier "the whole window is dead" reports.' },
			{ kind: 'fixed', summary: 'Resolved an intermittent pointer-blockage bug in the Dialog component that froze interactions after closing a modal.' },
			{ kind: 'fixed', summary: 'Version placeholders unified across files so dev and release builds no longer disagree about who they are.' },
		],
	},
	{
		version: '0.1.5',
		date: '2026-05-09',
		changes: [
			{ kind: 'added', summary: 'Linux screen capture: a Wayland-native pipeline using `xdg-desktop-portal` + PipeWire, and a parallel X11 native capture path. Linux recording docs refreshed alongside the new backends.' },
			{ kind: 'added', summary: 'Recording profiles: per-launch capture profiles with dynamic capability combinations, device awareness, and a management UI in Settings.' },
			{ kind: 'added', summary: 'Command palette (⌘K) extracted into a global `CommandPaletteHost` mounted at the root layout, so the shortcut and dialog work on every route — including the editor — not only on routes that render the sidebar.' },
			{ kind: 'added', summary: 'Web download page redesigned with new platform icons and a feature grid.' },
			{ kind: 'changed', summary: 'Properties panel: shared `PanelSection` primitive replaces ~30 ad-hoc section headers, drops repeated panel-name titles, normalises gap to `gap-4`, and standardises toggle / reset placement across Background, Focus, Annotations, Cursor, Audio, Camera, and Info panels.' },
			{ kind: 'changed', summary: 'Design tokens: introduced a Framer-inspired vocabulary (`canvas`, `surface-1/2`, `ink`, `ink-muted`, `hairline`, gradient spotlight cards, elevation shadows) layered on top of the existing shadcn tokens. Primary colour and font stack preserved.' },
		],
	},
	{
		version: '0.1.4',
		date: '2026-05-08',
		changes: [
			{ kind: 'added', summary: 'Camera overlay in the editor: composite the recorded camera track over the screen video with position presets, size, shape, and mirror toggles. Gated behind a `CAMERA_OVERLAY_UI_ENABLED` feature flag.' },
			{ kind: 'added', summary: 'Cursor: mouse-press events feed into the recorded timeline, and a refreshed set of cursor styles ships with the editor.' },
			{ kind: 'added', summary: 'Native macOS-style page transitions via the View Transitions API, with a smoother titlebar handoff between routes.' },
			{ kind: 'changed', summary: 'Canvas geometry and aspect-ratio handling: editor geometry helpers now carry the chosen aspect end-to-end (preview, composite, drop-shadow) without per-call ad-hoc math.' },
		],
	},
	{
		version: '0.1.3-beta',
		date: '2026-05-07',
		changes: [
			{ kind: 'added', summary: 'Active-preset chip in the editor toolbar with a reset-to-source affordance.' },
			{ kind: 'added', summary: 'Per-project preset persistence: applied preset and output aspect round-trip with undo/redo and project autosave.' },
			{ kind: 'changed', summary: 'GIF export now uses a 2-pass palettegen → paletteuse pipeline, so the progress bar advances in real time instead of sitting at 0% while only the elapsed counter ticked.' },
			{ kind: 'changed', summary: 'Presets actually resize the canvas to their target aspect (16:9, 9:16, 1:1, 1.91:1) end-to-end through the preview, FFmpeg filter graph, cursor overlay, and drop-shadow rasteriser.' },
			{ kind: 'changed', summary: 'Stronger blur annotation: redacts content even at full strength, with scaled tint opacity and an optional gray wash above 0.6 strength.' },
			{ kind: 'changed', summary: 'FFmpeg error reporting filters out progress noise so real diagnostic lines reach the failure toast.' },
			{ kind: 'fixed', summary: 'Region picker "Use area" / "Cancel" buttons now work; closing the main window exits the app instead of leaving aux windows holding the process.' },
			{ kind: 'fixed', summary: 'Quick action no longer opens the camera preview inside the recording panel window.' },
		],
	},
	{
		version: '0.1.2-beta',
		date: '2026-05-06',
		changes: [
			{ kind: 'added', summary: 'Timeline workspace: clip bar, playhead, ruler, toolbar, and zoom lane components.' },
			{ kind: 'added', summary: 'Blur annotations with adjustable strength, rendered through the composite canvas pipeline.' },
			{ kind: 'added', summary: 'Cursor animation effects: click bounce, idle sway, and motion blur.' },
			{ kind: 'added', summary: 'Glass card and chip components for a more refined UI surface.' },
			{ kind: 'added', summary: '`Kbd` component for consistent keyboard shortcut hints.' },
			{ kind: 'added', summary: 'Region selection in the source picker, with last-used source persistence.' },
			{ kind: 'added', summary: 'Camera overlay settings and validation, plus browser-based camera enumeration.' },
			{ kind: 'added', summary: 'Command palette (⌘K) with global navigation, recording, theme and external commands.' },
			{ kind: 'added', summary: 'Sidebar pinning and hover behavior.' },
			{ kind: 'changed', summary: 'Refactored project structure for readability and maintainability.' },
			{ kind: 'changed', summary: 'Upgraded Node.js to v24 and enabled `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24`.' },
			{ kind: 'changed', summary: 'Redesigned loading screen with new logo and progress bar.' },
			{ kind: 'changed', summary: 'Polished typography, spacing, and accessibility across annotation panels and headers.' },
			{ kind: 'fixed', summary: 'Reverted erroneous app version bump; settings layout regressions cleaned up.' },
		],
	},
	{
		version: '0.1.0-beta',
		date: 'Initial beta',
		changes: [
			{ kind: 'changed', summary: 'First public beta of Recast: offline-first desktop screen recorder and editor' },
		],
	},
] as const;
// RELEASES:END

export const LATEST_RELEASE = RELEASES[0];
