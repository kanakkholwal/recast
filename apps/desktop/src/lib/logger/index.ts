/**
 * Recast-scoped structured logger for the desktop editor. Tags records with the
 * open recast and fans out to the dev console (debug builds only) and the rotating
 * log file via `@tauri-apps/plugin-log`, which interleaves with the Rust backend's
 * logs in one support-bundle file.
 *
 * Gating (see `./diagnostics.svelte`): debug builds log everything; release builds
 * always persist `warn`/`error`, but `debug`/`info` only when the user enables
 * Diagnostic logging in Settings.
 *
 * High-frequency inputs (slider drags, scrubbing) must use `debounced` to coalesce
 * into one counted line instead of flooding the file.
 *
 * LOCAL only. PostHog is handled separately by `$lib/analytics`.
 */

import { diagnostics } from "./diagnostics.svelte";

type Level = "debug" | "info" | "warn" | "error";

const IS_DEV = import.meta.env.DEV;

interface RecastScope {
	/** Short stable hash of the project path, distinguishing recasts in a log. */
	id: string;
	/** Human label (the file's basename) for quick reading. */
	label: string;
}

// Per-window scope: each editor window is its own webview, so it holds exactly the recast it has open.
let scope: RecastScope | null = null;

/** Verbose (`debug`/`info`) logging is on in dev or when the user opted in. */
function verboseEnabled(): boolean {
	return IS_DEV || diagnostics.enabled;
}

// formatting

function basename(path: string): string {
	const p = path.replace(/\\/g, "/");
	const name = p.slice(p.lastIndexOf("/") + 1);
	return name || "recast";
}

/** FNV-1a → base36, trimmed. Stable per path, and avoids logging the full path. */
function shortId(path: string): string {
	let h = 0x811c9dc5;
	for (let i = 0; i < path.length; i++) {
		h ^= path.charCodeAt(i);
		h = Math.imul(h, 0x01000193);
	}
	return (h >>> 0).toString(36).padStart(6, "0").slice(0, 6);
}

function safeData(data?: Record<string, unknown>): string {
	if (!data) return "";
	const keys = Object.keys(data);
	if (keys.length === 0) return "";
	try {
		return ` ${JSON.stringify(data)}`;
	} catch {
		return ` {unserializable}`;
	}
}

function format(area: string, event: string, data?: Record<string, unknown>): string {
	const head = scope ? `[${scope.label}#${scope.id}]` : "[recast]";
	return `${head} ${area} · ${event}${safeData(data)}`;
}

// sinks

function toConsole(level: Level, msg: string): void {
	if (!IS_DEV) return;
	const fn =
		level === "error"
			? console.error
			: level === "warn"
				? console.warn
				: level === "debug"
					? console.debug
					: console.info;
	fn(msg);
}

// Lazily loaded so a non-Tauri preview degrades to console-only instead of throwing at import time.
let logPlugin: typeof import("@tauri-apps/plugin-log") | null = null;
let logPluginFailed = false;

async function toFile(level: Level, msg: string): Promise<void> {
	// warn and error are never verbose, so always persist them; debug and info only in dev or with diagnostics on.
	const always = level === "warn" || level === "error";
	if (!always && !verboseEnabled()) return;
	if (logPluginFailed) return;
	try {
		if (!logPlugin) logPlugin = await import("@tauri-apps/plugin-log");
		await logPlugin[level](msg);
	} catch {
		// Not running under Tauri, so stop retrying the dynamic import.
		logPluginFailed = true;
	}
}

function emit(level: Level, area: string, event: string, data?: Record<string, unknown>): void {
	const msg = format(area, event, data);
	toConsole(level, msg);
	void toFile(level, msg);
}

// debounced (high-frequency inputs)

interface PendingLog {
	timer: ReturnType<typeof setTimeout>;
	count: number;
	area: string;
	event: string;
	data?: Record<string, unknown>;
}

const pending = new Map<string, PendingLog>();
const DEFAULT_DEBOUNCE_MS = 400;

function flush(key: string): void {
	const entry = pending.get(key);
	if (!entry) return;
	pending.delete(key);
	const data = entry.count > 1 ? { ...entry.data, coalesced: entry.count } : entry.data;
	emit("debug", entry.area, entry.event, data);
}

// public API

export const log = {
	/**
	 * Bind every subsequent log in this window to a recast. Call when the editor
	 * opens a project; pass extra context (duration/dimensions) as `meta`.
	 */
	setRecast(path: string, meta?: Record<string, unknown>): void {
		scope = { id: shortId(path), label: basename(path) };
		emit("info", "session", "recast_opened", meta);
	},

	/** Clear the recast binding (editor closed/destroyed). */
	clearRecast(): void {
		if (scope) emit("info", "session", "recast_closed");
		scope = null;
		// Drop any not-yet-flushed debounced lines for the closing session.
		for (const [key, entry] of pending) {
			clearTimeout(entry.timer);
			pending.delete(key);
		}
	},

	debug(area: string, event: string, data?: Record<string, unknown>): void {
		emit("debug", area, event, data);
	},
	/** A noteworthy user action or milestone (default level for interactions). */
	info(area: string, event: string, data?: Record<string, unknown>): void {
		emit("info", area, event, data);
	},
	warn(area: string, event: string, data?: Record<string, unknown>): void {
		emit("warn", area, event, data);
	},
	error(area: string, event: string, data?: Record<string, unknown>): void {
		emit("error", area, event, data);
	},

	/**
	 * Coalesce rapid repeats of the SAME logical action (slider drag, scrub) into
	 * one trailing `debug` line, annotated with `coalesced` when >1 call merged.
	 * Keyed so distinct controls don't collapse into each other. Always pass the
	 * LATEST value as `data`: the last call before the quiet period wins.
	 */
	debounced(
		key: string,
		area: string,
		event: string,
		data?: Record<string, unknown>,
		waitMs: number = DEFAULT_DEBOUNCE_MS,
	): void {
		const existing = pending.get(key);
		if (existing) {
			clearTimeout(existing.timer);
			existing.count += 1;
			existing.data = data;
			existing.area = area;
			existing.event = event;
			existing.timer = setTimeout(() => flush(key), waitMs);
			return;
		}
		pending.set(key, {
			count: 1,
			area,
			event,
			data,
			timer: setTimeout(() => flush(key), waitMs),
		});
	},
};

export type Logger = typeof log;
