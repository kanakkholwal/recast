/**
 * The desktop implementation of {@link EditorServices}: every capability the
 * editor asks for, backed by `$lib/ipc` and the Tauri plugins. This is the only
 * place under the editor that knows Tauri exists.
 */

import { convertFileSrc } from "@tauri-apps/api/core";
import {
	captionCapabilities,
	deleteCaptionModel,
	detectSilence,
	downloadCaptionModel,
	ensureGoogleFont,
	exportCaptions,
	exportScreenText,
	extractWaveform,
	generateThumbnails,
	getVideoMetadata,
	hasTranscribableAudio,
	listCaptionModels,
	openFileLocation,
	readVideoText,
	suggestZoomRegions,
	transcribeProject,
} from "$lib/ipc";
import type { EditorServices, PickFileOptions } from "./services";

/** Assets that are already loadable stay untouched; only real paths go through
 *  the asset protocol. */
function resolveAssetUrl(ref: string): string {
	if (!ref) return ref;
	if (/^(data|blob|https?|asset|tauri):/i.test(ref)) return ref;
	return convertFileSrc(ref);
}

async function pickFile(opts: PickFileOptions): Promise<string | null> {
	const { open } = await import("@tauri-apps/plugin-dialog");
	const selected = await open({
		multiple: false,
		directory: false,
		title: opts.title,
		filters: [{ name: "Files", extensions: opts.accept }],
	});
	return typeof selected === "string" ? selected : null;
}

/** Ask for a destination path. The backend owns every actual write, so each
 *  caller pairs this with its own Rust command. */
async function pickSavePath(defaultName: string, accept: string[]): Promise<string | null> {
	const { save } = await import("@tauri-apps/plugin-dialog");
	const dest = await save({
		defaultPath: defaultName,
		filters: [{ name: accept.join("/").toUpperCase(), extensions: accept }],
	});
	return dest ?? null;
}

export const tauriEditorServices: EditorServices = {
	resolveAssetUrl,
	pickFile,
	captionFiles: {
		async exportSidecar(transcript, format) {
			const dest = await pickSavePath(`captions.${format}`, [format]);
			if (dest) await exportCaptions(transcript, format, dest);
		},
	},
	transcription: {
		capabilities: captionCapabilities,
		listModels: listCaptionModels,
		downloadModel: downloadCaptionModel,
		deleteModel: deleteCaptionModel,
		hasTranscribableAudio,
		transcribe: transcribeProject,
	},
	analysis: {
		detectSilence: (a) => detectSilence(a.audioPath, a.microphonePath, a.cursorPath, a.options),
		suggestZoomRegions,
	},
	mediaAnalysis: {
		waveform: (a) => extractWaveform(a.audioPath, a.microphonePath, a.buckets),
		thumbnails: generateThumbnails,
		videoMetadata: getVideoMetadata,
	},
	assets: {
		googleFont: ensureGoogleFont,
	},
	shell: {
		openFileLocation,
		openExternal: async (url) => {
			const { openUrl } = await import("@tauri-apps/plugin-opener");
			await openUrl(url);
		},
	},
	ocr: {
		readVideoText,
		async exportScreenText(body, defaultName) {
			const ext = defaultName.split(".").pop() ?? "json";
			const dest = await pickSavePath(defaultName, [ext]);
			if (dest) await exportScreenText(body, dest);
		},
	},
};
