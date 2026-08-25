/**
 * Experimental-feature flags, persisted to localStorage and off by default.
 * Add one by extending `ExperimentalFlag` + `DEFAULTS`; the settings page
 * renders a row per flag from `FLAG_META`.
 */

import { PersistedState } from "@recast/ui/persisted-state";

export type ExperimentalFlag =
	| "silenceDetection"
	| "selfHosting"
	| "remoteTranscription"
	| "browserExportBeta"
	| "wasmPreviewEngine";

interface FlagMeta {
	key: ExperimentalFlag;
	label: string;
	description: string;
}

export const FLAG_META: FlagMeta[] = [
	{
		key: "silenceDetection",
		label: "Silence detection & cuts",
		description:
			"Find quiet stretches with no cursor movement and skip them on playback and export.",
	},
	{
		key: "selfHosting",
		label: "Self-hosting server endpoint",
		description:
			"Point the app at your own Recast Cloud server. Cloud isn't ready yet, so this is for early self-hosters only.",
	},
	{
		key: "remoteTranscription",
		label: "Remote transcription endpoints",
		description:
			"Transcribe captions through an OpenAI-compatible endpoint (LM Studio, a self-hosted server, or a third-party API) instead of an on-device model. Response formats vary between servers, so treat this as early.",
	},
	{
		key: "wasmPreviewEngine",
		label: "New preview engine (beta)",
		description:
			"Composite the preview with the new Rust engine on WebGPU instead of WebGL2. The same code renders the export, so what you see is what you get. Early — falls back to WebGL2 if your device has no WebGPU.",
	},
	{
		key: "browserExportBeta",
		label: "New export engine (beta)",
		description:
			"Render exports through the new browser engine so they match the preview exactly, instead of the FFmpeg compositor. Falls back automatically if your device can't. Early — compare an export before relying on it.",
	},
];

const DEFAULTS: Record<ExperimentalFlag, boolean> = {
	silenceDetection: false,
	selfHosting: false,
	remoteTranscription: false,
	browserExportBeta: false,
	wasmPreviewEngine: false,
};

const STORAGE_KEY = "recast-experimental-flags";

function createExperimentalStore() {
	// Merges saved JSON over DEFAULTS so adding a flag later keeps existing
	// choices. Tauri v2 webviews share a localStorage origin, so a flip in the
	// settings window reaches open editor windows without a reload.
	const flags = new PersistedState<Record<ExperimentalFlag, boolean>>(STORAGE_KEY, DEFAULTS);

	return {
		get silenceDetection() {
			return flags.current.silenceDetection;
		},
		isEnabled(key: ExperimentalFlag): boolean {
			return flags.current[key];
		},
		setEnabled(key: ExperimentalFlag, value: boolean) {
			flags.current = { ...flags.current, [key]: value };
		},
	};
}

export const experimentalStore = createExperimentalStore();
