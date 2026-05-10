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
