/**
 * Contracts for the agent-session listener. Deliberately transport-free: the
 * desktop host drives these off Tauri events, a future web host off SSE or a
 * websocket, and neither shape leaks into the editor.
 */

import type { EditorRenderState } from "../editor/render-state";
import type { BranchDriver } from "./branches";

/** Which host is feeding the listener. Web is reserved; no driver ships yet. */
export type AgentSessionMode = "desktop" | "web";

/** Who holds the project write-lock. Mirrors Rust's `EditorWriterKind`. */
export type AgentWriter = "ui" | "agent";

/**
 * The project write-lock as the backend sees it. `writer: null` means nobody
 * holds it — the GUI is free to edit.
 */
export interface AgentSessionSnapshot {
	writer: AgentWriter | null;
	/** Opaque holder id (`agent:claude`, `ui:main`). Empty when unheld. */
	writerId: string;
	projectPath: string | null;
	acquiredAtMs: number;
	lastActivityAtMs: number;
}

export const IDLE_SESSION: AgentSessionSnapshot = {
	writer: null,
	writerId: "",
	projectPath: null,
	acquiredAtMs: 0,
	lastActivityAtMs: 0,
};

/** One line in the "what the agent just did" popover. */
export interface AgentActivity {
	id: string;
	atMs: number;
	summary: string;
}

export type AgentSessionEvent =
	| { type: "session"; session: AgentSessionSnapshot }
	| { type: "state-changed"; projectPath: string; summary?: string }
	| { type: "branches-changed"; projectPath: string };

/**
 * Host-supplied transport. Omitting the driver leaves the listener permanently
 * idle, which is exactly what a host without an agent backend should get —
 * never a throw.
 */
export interface AgentSessionDriver {
	readonly mode: AgentSessionMode;
	/** Resolves to an unsubscribe fn. Called once per bound project. */
	subscribe(sink: (event: AgentSessionEvent) => void): Promise<() => void>;
	getSession(): Promise<AgentSessionSnapshot>;
	/** Re-read the authoritative on-disk edits for a project. */
	readRenderState(projectPath: string): Promise<Partial<EditorRenderState>>;
	/** Evict the current holder so the user can take over. */
	releaseControl?(): Promise<void>;
	/** Branch review. Omitted by hosts without a journal backend. */
	readonly branches?: BranchDriver;
}
