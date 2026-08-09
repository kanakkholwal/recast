/**
 * The desktop implementation of {@link EditorServices}: every capability the
 * editor asks for, backed by `$lib/ipc` and the Tauri plugins. This is the only
 * place under the editor that knows Tauri exists.
 */

import { convertFileSrc } from "@tauri-apps/api/core";
import {
	cancelTranscription,
	captionCapabilities,
	deleteCaptionModel,
	detectSilence,
	downloadCaptionModel,
	ensureAssetsInstalled,
	ensureGoogleFont,
	enqueueExport,
	exportCaptions,
	exportScreenText,
	extractWaveform,
	fetchExtensionRegistry,
	generateThumbnails,
	getCachedAssetPath,
	getVideoMetadata,
	hasTranscribableAudio,
	hydrateCachedAssets,
	installExtension,
	listCaptionModels,
	listInstalledExtensions,
	openFileLocation,
	readVideoText,
	saveBrowserExportVideo,
	setExtensionEnabled,
	suggestZoomRegions,
	transcribeProject,
	uninstallExtension,
} from "$lib/ipc";
import type { EditorServices, PickFileOptions } from "@recast/editor/services";

/** Assets that are already loadable stay untouched; only real paths go through
 *  the asset protocol. */
function resolveAssetUrl(ref: string): string {
	if (!ref) return ref;
	if (/^(data|blob|https?|asset|tauri):/i.test(ref)) return ref;
	return convertFileSrc(ref);
}

/** The view's bytes as a standalone `ArrayBuffer`, copying only when it really
 *  is a window onto a larger buffer. An export mp4 is GBs; an unconditional
 *  `slice` doubled peak memory to no purpose. */
function exactBuffer(view: Uint8Array): ArrayBuffer {
	if (view.byteOffset === 0 && view.byteLength === view.buffer.byteLength) {
		return view.buffer as ArrayBuffer;
	}
	return view.buffer.slice(view.byteOffset, view.byteOffset + view.byteLength) as ArrayBuffer;
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
		cancel: cancelTranscription,
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
		ensureInstalled: ensureAssetsInstalled,
		getCachedPath: getCachedAssetPath,
		hydrate: hydrateCachedAssets,
	},
	extensions: {
		fetchRegistry: fetchExtensionRegistry,
		install: installExtension,
		listInstalled: listInstalledExtensions,
		setEnabled: setExtensionEnabled,
		uninstall: uninstallExtension,
	},
	exportSink: {
		// Rust muxes the audio into this video-only mp4 (`-c:v copy`), so the
		// bytes land in a temp file rather than coming back as a Blob.
		deliver: (bytes) => saveBrowserExportVideo(exactBuffer(bytes)),
		enqueue: (job) => enqueueExport(job as Parameters<typeof enqueueExport>[0]),
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
