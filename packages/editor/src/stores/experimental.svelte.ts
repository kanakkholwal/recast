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
	| "engineExport";

interface FlagMeta {
	key: ExperimentalFlag;
	label: string;
	description: string;
}

export const FLAG_META: FlagMeta[] = [
	{
		key: "silenceDetection",
		label: "Silence detection & cuts (alpha)",
		description:
			"Find quiet stretches with no cursor movement and skip them on playback and export. Alpha: the detector is being replaced, so where it puts a cut will change.",
	},
	{
		key: "selfHosting",
		label: "Self-hosting server endpoint (beta)",
		description:
			"Point the app at your own Recast Cloud server instead of the hosted one. Beta: usable, but endpoints and stored shapes can still change between releases.",
	},
	{
		key: "remoteTranscription",
		label: "Remote transcription endpoints",
		description:
			"Transcribe captions through an OpenAI-compatible endpoint (LM Studio, a self-hosted server, or a third-party API) instead of an on-device model. Response formats vary between servers, so treat this as early.",
	},
	{
		key: "browserExportBeta",
		label: "Export in the preview window (beta)",
		description:
			"Run the engine in the preview window and encode with WebCodecs. Same renderer as the preview, so it matches exactly. Falls back automatically above 1080p60 and where WebCodecs is missing.",
	},
	{
		key: "engineExport",
		label: "Export in the background (alpha)",
		description:
			"Run the same engine natively instead of the FFmpeg compositor, with no resolution ceiling and no browser needed. Alpha: compare an export before relying on it, and quote the export log line if one looks wrong.",
	},
];

const DEFAULTS: Record<ExperimentalFlag, boolean> = {
	silenceDetection: false,
	selfHosting: false,
	remoteTranscription: false,
	browserExportBeta: false,
	engineExport: false,
};

const STORAGE_KEY = "recast-experimental-flags";

function createExperimentalStore() {
	// Merged over DEFAULTS so a new flag keeps existing choices; Tauri webviews share an origin, so a flip reaches open editors.
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
