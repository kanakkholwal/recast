/**
 * Desktop driver for the editor's agent-session listener. The web build will
 * ship a sibling that speaks HTTP/SSE; nothing under the editor branches on
 * which one is installed.
 */

import type { AgentSessionDriver, AgentSessionEvent, AgentSessionSnapshot } from "@recast/editor";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { loadEditorDocument } from "$lib/ipc";

/** Emitted by `commands::editor_session::commit` on every lock transition. */
const SESSION_EVENT = "editor-session:changed";
/** Emitted by `patch_render_state` after an agent writes edits to disk. */
const STATE_EVENT = "editor-state:changed";

export const tauriAgentSessionDriver: AgentSessionDriver = {
	mode: "desktop",

	async subscribe(sink: (event: AgentSessionEvent) => void) {
		const offSession = await listen<AgentSessionSnapshot>(SESSION_EVENT, ({ payload }) => {
			sink({ type: "session", session: payload });
		});
		const offState = await listen<{ path: string; summary?: string }>(
			STATE_EVENT,
			({ payload }) => {
				if (!payload?.path) return;
				sink({ type: "state-changed", projectPath: payload.path, summary: payload.summary });
			},
		);
		return () => {
			offSession();
			offState();
		};
	},

	getSession() {
		return invoke<AgentSessionSnapshot>("get_editor_session");
	},

	async readRenderState(projectPath: string) {
		const doc = await loadEditorDocument(projectPath);
		return doc.renderState;
	},

	async releaseControl() {
		await invoke("force_release_editor_write");
	},
};

/** Claim the project for the GUI. Rejects with `editor_locked: …` when an agent
 *  already holds it, which the caller surfaces rather than swallowing. */
export function acquireEditorWrite(projectPath: string, writerId: string): Promise<unknown> {
	return invoke("acquire_editor_write", { projectPath, writerId });
}

export function releaseEditorWrite(writerId: string): Promise<boolean> {
	return invoke<boolean>("release_editor_write", { writerId });
}
