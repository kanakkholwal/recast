/**
 * Public surface of the Recast editor. Hosts mount `<Editor />`, hand it a store
 * and an {@link EditorServices}, and get the whole non-linear editor — timeline,
 * WebGL preview, overlays, properties panels and the browser export compositor.
 */

export { default as BranchReviewPanel } from "./components/BranchReviewPanel.svelte";
export type { EditorProps, ToolbarControls } from "./Editor.svelte";
export { default as Editor } from "./Editor.svelte";
export { branchReview } from "./lib/agent/branch-store.svelte";
export type {
	AppendReport,
	ApplyReport,
	BranchDriver,
	BranchSummary,
	EditOp,
	EditOpTag,
	FieldChange,
} from "./lib/agent/branches";
export { changeGroup, describeChange, EDIT_OP_TAGS, groupChanges } from "./lib/agent/branches";
export type { BindOptions as AgentSessionBindOptions } from "./lib/agent/session.svelte";
export { agentSession, setAgentSessionDriver } from "./lib/agent/session.svelte";
export type {
	AgentActivity,
	AgentSessionDriver,
	AgentSessionEvent,
	AgentSessionMode,
	AgentSessionSnapshot,
	AgentWriter,
} from "./lib/agent/types";
export type { PanelTab } from "./lib/editor/panel-tabs";
export { PANEL_TABS, WEB_PANEL_TABS } from "./lib/editor/panel-tabs";
export * from "./lib/editor/render-state";
export * from "./lib/editor/services";
export * from "./lib/editor/track-offsets";
export type {
	EditorAnalytics,
	EditorWorkerName,
	ExportActivityHost,
	ShortcutHost,
	WorkerHost,
} from "./lib/host-hooks";
export { setEditorHostHooks } from "./lib/host-hooks";
export type { LogSink } from "./lib/log";
export { setLogSink } from "./lib/log";
export type { AudioTrackSpec } from "./lib/playback/audio-engine";
export { AudioTimelineEngine } from "./lib/playback/audio-engine";
export type { AudioEngineHolder } from "./lib/playback/audio-engine-host.svelte";
export { createAudioEngineHost } from "./lib/playback/audio-engine-host.svelte";
export type { EditorStore } from "./stores/editor-store.svelte";
export { createEditorStore } from "./stores/editor-store.svelte";

// Pure helpers hosts otherwise reach by deep @recast/editor/lib/* subpaths.
export * from "./lib/audio/music";
export * from "./lib/camera/browser-devices";
export * from "./lib/captions/clip-with-cuts";
export * from "./lib/dom/keyboard";
export * from "./lib/format/bytes";
export * from "./lib/format/files";
export * from "./lib/format/time";
export * from "./lib/format/transfer-rate";
export * from "./lib/profiles";
export * from "./lib/timeline/time-map";
