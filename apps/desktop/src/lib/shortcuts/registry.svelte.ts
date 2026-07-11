/**
 * Central keyboard-shortcut registry: single source of truth for every shortcut.
 *
 * Two jobs:
 *   1. `shortcutDefs` documents every shortcut so the Shortcuts dialog renders an
 *      always-accurate reference.
 *   2. A single window listener (`dispatchShortcut`) handles app/editor mod-combo
 *      shortcuts; components attach state-dependent handlers via
 *      `registerShortcutHandlers`. One dispatcher + one def per chord means two
 *      shortcuts can never fire from one press (the "Ctrl triggers everything" bug
 *      this was built to kill).
 *
 * Plain-key / focus-scoped shortcuts (annotation tools, timeline JKL, nudge, mute)
 * are deliberately NOT centrally dispatched: they reuse letters across contexts
 * and rely on focus/panel state to disambiguate. They live in their components
 * (each on its own `<svelte:window>` so HMR can't leak them) and appear here as
 * `central: false` for the dialog.
 */

import { commandPalette } from "$lib/stores/command-palette.svelte";

type Handler = () => void | Promise<void>;

export interface ShortcutDef {
	/** Stable id. Central defs are wired to a handler/action by this id. */
	id: string;
	/** Canonical chord, e.g. "Mod+Shift+Z", "Mod+]", "Space", "F". `Mod` is ⌘
	 *  on macOS and Ctrl elsewhere. Used for both matching and (by default)
	 *  display. */
	keys: string;
	/** Optional explicit display tokens, for shortcuts a single chord can't
	 *  express (e.g. the four arrow keys for nudge). Overrides `keys` rendering. */
	display?: string[];
	label: string;
	description?: string;
	/** Grouping heading in the Shortcuts dialog. */
	category: string;
	/** When true, the central dispatcher owns this chord (needs a handler or
	 *  `action`). When false/undefined it's documentation-only; its component
	 *  handles the key locally. */
	central?: boolean;
	/** Fire even when a text input / contenteditable is focused. Default false. */
	allowInInput?: boolean;
	/** Subtle qualifier shown in the dialog, e.g. "when timeline is focused". */
	scopeNote?: string;
	/** Default action for a central def with no component-registered handler
	 *  (used by globally-available actions like the command palette). */
	action?: Handler;
}

const IS_MAC =
	typeof navigator !== "undefined" &&
	/mac|iphone|ipad/i.test(navigator.platform || navigator.userAgent || "");

//  chord parsing / matching -

const MODIFIER_KEYS = new Set([
	"Control",
	"Shift",
	"Alt",
	"Meta",
	"OS",
	"AltGraph",
	"CapsLock",
]);

function normalizeKey(k: string): string {
	if (k === " " || k === "Space" || k === "Spacebar") return "Space";
	if (k.length === 1) return k.toUpperCase();
	return k; // ArrowLeft, Delete, Escape, Enter, Home, End, …
}

function canonical(mod: boolean, shift: boolean, alt: boolean, key: string): string {
	const parts: string[] = [];
	if (mod) parts.push("Mod");
	if (shift) parts.push("Shift");
	if (alt) parts.push("Alt");
	parts.push(normalizeKey(key));
	return parts.join("+");
}

function chordFromEvent(e: KeyboardEvent): string {
	const mod = IS_MAC ? e.metaKey : e.ctrlKey;
	return canonical(mod, e.shiftKey, e.altKey, e.key);
}

function chordFromKeys(keys: string): string {
	const segs = keys.split("+").map((s) => s.trim());
	const key = segs.pop() ?? "";
	return canonical(
		segs.includes("Mod"),
		segs.includes("Shift"),
		segs.includes("Alt"),
		key,
	);
}

//  display 

const KEY_GLYPHS: Record<string, string> = {
	Mod: IS_MAC ? "⌘" : "Ctrl",
	Shift: IS_MAC ? "⇧" : "Shift",
	Alt: IS_MAC ? "⌥" : "Alt",
	Space: "Space",
	Enter: "↵",
	Escape: "Esc",
	Delete: "Del",
	Backspace: "⌫",
	ArrowLeft: "←",
	ArrowRight: "→",
	ArrowUp: "↑",
	ArrowDown: "↓",
	Home: "Home",
	End: "End",
};

/** Turn a canonical chord ("Mod+Shift+Z") into display tokens (["⌘","⇧","Z"]). */
export function formatChordTokens(keys: string): string[] {
	return keys
		.split("+")
		.map((s) => s.trim())
		.map((tok) => KEY_GLYPHS[tok] ?? (tok.length === 1 ? tok.toUpperCase() : tok));
}

/**
 * Display label for a registered shortcut by id, platform-aware and always in
 * sync with its real binding (e.g. "⌘⌥R" on macOS, "Ctrl+Alt+R" elsewhere).
 * Returns "" for an unknown id. Prefer this over hardcoding chords in the UI.
 */
export function chordLabel(id: string): string {
	const def = shortcutDefs.find((d) => d.id === id);
	if (!def) return "";
	const tokens = def.display ?? formatChordTokens(def.keys);
	return IS_MAC ? tokens.join("") : tokens.join("+");
}

//  the master list --

export const shortcutDefs: ShortcutDef[] = [
	// General: available everywhere.
	{
		id: "general.palette",
		keys: "Mod+K",
		label: "Command palette",
		description: "Search and run any command",
		category: "General",
		central: true,
		allowInInput: true,
		action: () => commandPalette.toggle(),
	},
	{
		id: "general.shortcuts",
		keys: "Mod+/",
		label: "Keyboard shortcuts",
		description: "Open this reference",
		category: "General",
		central: true,
		allowInInput: true,
		action: () => shortcutsDialog.toggle(),
	},
	{
		id: "general.record",
		keys: "Mod+Alt+R",
		label: "Launch recording panel",
		category: "General",
		central: true,
		action: async () => {
			const { launchRecordingPanel } = await import("$lib/ipc");
			await launchRecordingPanel();
		},
	},

	// Global (OS-wide) recording hotkeys, registered by the Rust global-shortcut
	// plugin, not the window dispatcher (hence central: false). Listed here so
	// the Shortcuts dialog documents them.
	{
		id: "global.recordToggle",
		keys: "Alt+Shift+R",
		label: "Start / stop recording",
		category: "General",
		scopeNote: "Works anywhere, even when Recast is in the background",
	},
	{
		id: "global.pauseToggle",
		keys: "Alt+Shift+P",
		label: "Pause / resume recording",
		category: "General",
		scopeNote: "While recording, from any app",
	},

	// Editing: editor route registers these handlers.
	{ id: "editor.undo", keys: "Mod+Z", label: "Undo", category: "Editing", central: true },
	{ id: "editor.redo", keys: "Mod+Shift+Z", label: "Redo", category: "Editing", central: true },
	{ id: "editor.save", keys: "Mod+S", label: "Save", category: "Editing", central: true },

	// View: editor route.
	{
		id: "editor.toggleSidebar",
		keys: "Mod+B",
		label: "Toggle properties panel",
		category: "View",
		central: true,
	},
	{
		id: "editor.toggleTimeline",
		keys: "Mod+J",
		label: "Toggle timeline",
		category: "View",
		central: true,
	},
	{
		id: "editor.presets",
		keys: "Mod+P",
		label: "Export presets",
		category: "View",
		central: true,
	},
	{
		id: "editor.fullscreen",
		keys: "F",
		label: "Toggle fullscreen preview",
		category: "View",
	},

	// Playback: editor route (plain keys, kept local).
	{ id: "editor.playPause", keys: "Space", label: "Play / pause", category: "Playback" },
	{
		id: "editor.prevFrame",
		keys: "ArrowLeft",
		label: "Previous frame",
		description: "Hold Shift for a 1-frame step",
		category: "Playback",
	},
	{ id: "editor.nextFrame", keys: "ArrowRight", label: "Next frame", category: "Playback" },

	// Annotation tools: active on the Annotations tab.
	{ id: "tool.select", keys: "V", label: "Select tool", category: "Annotation tools" },
	{ id: "tool.rect", keys: "R", label: "Rectangle", category: "Annotation tools" },
	{ id: "tool.ellipse", keys: "O", label: "Ellipse", category: "Annotation tools" },
	{ id: "tool.arrow", keys: "A", label: "Arrow", category: "Annotation tools" },
	{ id: "tool.text", keys: "T", label: "Text", category: "Annotation tools" },
	{ id: "tool.blur", keys: "B", label: "Blur", category: "Annotation tools" },

	// Annotations: with a selection.
	{ id: "anno.delete", keys: "Delete", label: "Delete annotation", category: "Annotations" },
	{ id: "anno.deselect", keys: "Escape", label: "Deselect / cancel tool", category: "Annotations" },
	{
		id: "anno.duplicate",
		keys: "Mod+D",
		label: "Duplicate annotation",
		category: "Annotations",
	},
	{ id: "anno.forward", keys: "Mod+]", label: "Bring forward", category: "Annotations" },
	{ id: "anno.backward", keys: "Mod+[", label: "Send backward", category: "Annotations" },
	{
		id: "anno.nudge",
		keys: "Arrows",
		display: ["←", "→", "↑", "↓"],
		label: "Nudge position",
		description: "Hold Shift for 10px",
		category: "Annotations",
		scopeNote: "when selected",
	},

	// Audio: active on the Audio tab.
	{ id: "audio.mute", keys: "M", label: "Toggle mute", category: "Audio" },

	// Timeline: when the timeline has focus.
	{ id: "timeline.in", keys: "I", label: "Set in point", category: "Timeline", scopeNote: "timeline focused" },
	{ id: "timeline.out", keys: "O", label: "Set out point", category: "Timeline", scopeNote: "timeline focused" },
	{ id: "timeline.reverse", keys: "J", label: "Shuttle reverse", category: "Timeline", scopeNote: "timeline focused" },
	{ id: "timeline.stop", keys: "K", label: "Shuttle stop", category: "Timeline", scopeNote: "timeline focused" },
	{ id: "timeline.forward", keys: "L", label: "Shuttle forward", category: "Timeline", scopeNote: "timeline focused" },
	{ id: "timeline.home", keys: "Home", label: "Jump to in point", category: "Timeline", scopeNote: "timeline focused" },
	{ id: "timeline.end", keys: "End", label: "Jump to out point", category: "Timeline", scopeNote: "timeline focused" },
	{ id: "timeline.trimIn", keys: "Alt+[", label: "Trim in point", category: "Timeline", scopeNote: "timeline focused" },
	{ id: "timeline.trimOut", keys: "Alt+]", label: "Trim out point", category: "Timeline", scopeNote: "timeline focused" },
	{ id: "timeline.paste", keys: "Mod+V", label: "Paste region", category: "Timeline", scopeNote: "timeline focused" },

	// Navigation: the app sidebar (library routes). Same chord as the editor's
	// properties panel, but a different route, so they never collide at runtime.
	{ id: "app.sidebar", keys: "Mod+B", label: "Toggle sidebar", category: "Navigation" },
];

/** Defs grouped by category, in declaration order. For the dialog. */
export function shortcutsByCategory(): [string, ShortcutDef[]][] {
	const map = new Map<string, ShortcutDef[]>();
	for (const def of shortcutDefs) {
		if (!map.has(def.category)) map.set(def.category, []);
		map.get(def.category)!.push(def);
	}
	return Array.from(map.entries());
}

//  central dispatch -

const centralByChord = new Map<string, ShortcutDef>();
for (const def of shortcutDefs) {
	if (def.central) centralByChord.set(chordFromKeys(def.keys), def);
}

// Component-provided handlers, keyed by shortcut id. A def's handler takes
// precedence over its default `action`; absent both, the chord is inert (e.g.
// an editor shortcut while no editor is open).
const handlers = new Map<string, Handler>();

/**
 * Register handlers for central shortcuts; returns a disposer to run on destroy.
 * Deletion is identity-checked so a remount race can't unregister a newer handler.
 */
export function registerShortcutHandlers(map: Record<string, Handler>): () => void {
	const entries = Object.entries(map);
	for (const [id, fn] of entries) handlers.set(id, fn);
	return () => {
		for (const [id, fn] of entries) {
			if (handlers.get(id) === fn) handlers.delete(id);
		}
	};
}

function isEditableTarget(t: EventTarget | null): boolean {
	const el = t as HTMLElement | null;
	if (!el || !el.tagName) return false;
	const tag = el.tagName;
	return (
		tag === "INPUT" ||
		tag === "TEXTAREA" ||
		tag === "SELECT" ||
		el.isContentEditable === true
	);
}

/**
 * Single window-level dispatcher for central shortcuts. Wire ONCE via
 * `<svelte:window onkeydown={dispatchShortcut} />` in the root layout.
 */
export function dispatchShortcut(e: KeyboardEvent): void {
	if (e.defaultPrevented || e.repeat) return;
	if (MODIFIER_KEYS.has(e.key)) return;
	const def = centralByChord.get(chordFromEvent(e));
	if (!def) return;
	if (!def.allowInInput && isEditableTarget(e.target)) return;
	const handler = handlers.get(def.id) ?? def.action;
	if (!handler) return; // no active handler for this context
	e.preventDefault();
	void handler();
}

//  shortcuts dialog open-state -

class ShortcutsDialogState {
	open = $state(false);
	toggle() {
		this.open = !this.open;
	}
	show() {
		this.open = true;
	}
	hide() {
		this.open = false;
	}
}

export const shortcutsDialog = new ShortcutsDialogState();
