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
export const RELEASES: readonly ChangelogRelease[] = [
	{
		version: "0.1.2-beta",
		date: "2026-05-06",
		title: "Timeline, blur, and a sharper editor",
		highlights: [
			"Editable timeline workspace with playhead and zoom lane",
			"Blur annotations with adjustable strength",
			"Cursor animation effects (bounce, sway, motion blur)",
		],
		changes: [
			{ kind: "added", summary: "Timeline: clip bar, playhead, ruler, toolbar, and zoom lane." },
			{ kind: "added", summary: "Blur annotations rendered through the composite canvas." },
			{ kind: "added", summary: "Cursor effects: click bounce, idle sway, and motion blur." },
			{ kind: "added", summary: "Glass card and chip components for richer surfaces." },
			{ kind: "added", summary: "Kbd component for consistent shortcut hints." },
			{ kind: "added", summary: "Region selection in source picker, with last-used source persisted." },
			{ kind: "added", summary: "Camera overlay settings and browser-based camera enumeration." },
			{ kind: "added", summary: "Command palette (⌘K) for navigation, recording, and theme." },
			{ kind: "added", summary: "Sidebar pinning and hover behavior." },
			{ kind: "changed", summary: "Refactored project structure for readability and maintainability." },
			{ kind: "changed", summary: "Upgraded Node.js to v24." },
			{ kind: "changed", summary: "Redesigned loading screen with new logo and progress bar." },
			{ kind: "changed", summary: "Polished typography, spacing, and accessibility." },
			{ kind: "fixed", summary: "Reverted erroneous app version bump; settings layout cleaned up." },
		],
	},
	{
		version: "0.1.0-beta",
		date: "2026-01-15",
		title: "Initial public beta",
		highlights: [
			"Offline-first desktop recorder and editor",
			"Built on Tauri v2, Svelte 5, and Rust",
		],
		changes: [
			{ kind: "added", summary: "First public beta of Recast." },
		],
	},
] as const;

export const LATEST_RELEASE = RELEASES[0];
