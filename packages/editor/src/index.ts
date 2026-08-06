/**
 * Public surface of the Recast editor. Hosts mount `<Editor />`, hand it a store
 * and an {@link EditorServices}, and get the whole non-linear editor — timeline,
 * WebGL preview, overlays, properties panels and the browser export compositor.
 */

export { default as Editor } from "./Editor.svelte";
export type { EditorProps } from "./Editor.svelte";

export { createEditorStore } from "./stores/editor-store.svelte";
export type { EditorStore } from "./stores/editor-store.svelte";

export * from "./lib/editor/render-state";
export * from "./lib/editor/services";
export { PANEL_TABS, WEB_PANEL_TABS } from "./lib/editor/panel-tabs";
export type { PanelTab } from "./lib/editor/panel-tabs";
export { setEditorHostHooks } from "./lib/host-hooks";
export type {
	EditorAnalytics,
	EditorWorkerName,
	ExportActivityHost,
	ShortcutHost,
	WorkerHost,
} from "./lib/host-hooks";

export { agentSession, setAgentSessionDriver } from "./lib/agent/session.svelte";
export type { BindOptions as AgentSessionBindOptions } from "./lib/agent/session.svelte";
export type {
	AgentActivity,
	AgentSessionDriver,
	AgentSessionEvent,
	AgentSessionMode,
	AgentSessionSnapshot,
	AgentWriter,
} from "./lib/agent/types";
export { setLogSink } from "./lib/log";
export type { LogSink } from "./lib/log";
