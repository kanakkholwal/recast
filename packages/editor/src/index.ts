/**
 * Public surface of the Recast editor. Hosts mount `<Editor />`, hand it a store
 * and an {@link EditorServices}, and get the whole non-linear editor — timeline,
 * WebGL preview, overlays, properties panels and the browser export compositor.
 */

export { default as Editor } from "./Editor.svelte";
export type { EditorProps, ToolbarControls } from "./Editor.svelte";

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
export { default as BranchReviewPanel } from "./components/BranchReviewPanel.svelte";
export { branchReview } from "./lib/agent/branch-store.svelte";
export { changeGroup, describeChange, EDIT_OP_TAGS, groupChanges } from "./lib/agent/branches";
export type {
	AppendReport,
	ApplyReport,
	BranchDriver,
	BranchSummary,
	EditOp,
	EditOpTag,
	FieldChange,
} from "./lib/agent/branches";
export { createAudioEngineHost } from "./lib/playback/audio-engine-host.svelte";
export type { AudioEngineHolder } from "./lib/playback/audio-engine-host.svelte";
export { AudioTimelineEngine } from "./lib/playback/audio-engine";
export type { AudioTrackSpec } from "./lib/playback/audio-engine";
export { setLogSink } from "./lib/log";
export type { LogSink } from "./lib/log";
