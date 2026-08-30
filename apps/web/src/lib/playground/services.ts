// biome-ignore-all lint/suspicious/useAwait: EditorServices declares these methods as Promise-returning; the browser host has nothing to await.
/**
 * The browser implementation of {@link EditorServices}. Every native capability
 * is simply absent, so the editor hides those surfaces rather than offering
 * buttons that fail: no on-device ASR, no silence/auto-zoom analysis, no local
 * asset installs, no OCR.
 */

import type { EditorServices, Transcript } from "@recast/editor/services";
import { parseSubtitles, transcriptToVtt } from "./captions-import";

/** Refs are already loadable here (object URLs, data URLs, https). */
const resolveAssetUrl = (ref: string) => ref;

function download(blob: Blob, name: string): void {
	const url = URL.createObjectURL(blob);
	const a = document.createElement("a");
	a.href = url;
	a.download = name;
	a.click();
	// Revoke on the next task: revoking synchronously can cancel the download in some browsers before it starts.
	setTimeout(() => URL.revokeObjectURL(url), 0);
}

/** `<input type=file>` in place of a native picker; the object URL IS the ref. */
function pickFile(opts: { accept: string[]; title?: string }): Promise<string | null> {
	return new Promise((resolve) => {
		const input = document.createElement("input");
		input.type = "file";
		input.accept = opts.accept.map((e) => `.${e}`).join(",");
		input.onchange = () => {
			const file = input.files?.[0];
			resolve(file ? URL.createObjectURL(file) : null);
		};
		// A cancelled picker fires no `change` in older browsers; `cancel` covers it.
		input.oncancel = () => resolve(null);
		input.click();
	});
}

function pickSubtitleFile(): Promise<Transcript | null> {
	return new Promise((resolve) => {
		const input = document.createElement("input");
		input.type = "file";
		input.accept = ".srt,.vtt";
		input.onchange = async () => {
			const file = input.files?.[0];
			if (!file) return resolve(null);
			try {
				resolve(parseSubtitles(await file.text(), file.name));
			} catch {
				resolve(null);
			}
		};
		input.oncancel = () => resolve(null);
		input.click();
	});
}

export const webEditorServices: EditorServices = {
	resolveAssetUrl,
	pickFile,
	captionFiles: {
		async exportSidecar(transcript, format) {
			// SRT and VTT differ only in the timestamp separator and the header.
			const vtt = transcriptToVtt(transcript);
			const body = format === "vtt" ? vtt : vttToSrt(vtt);
			download(new Blob([body], { type: "text/plain" }), `captions.${format}`);
		},
		importSidecar: pickSubtitleFile,
	},
	exportSink: {
		async deliver(bytes, suggestedName) {
			download(new Blob([bytes as BlobPart], { type: "video/mp4" }), suggestedName);
			return null;
		},
	},
	shell: {
		async openFileLocation() {
			// No filesystem here; the Info panel hides the affordance instead.
		},
		async openExternal(url) {
			window.open(url, "_blank", "noopener,noreferrer");
		},
	},
};

/** VTT → SRT: numbered cues and a comma decimal separator. */
function vttToSrt(vtt: string): string {
	const cues = vtt
		.replace(/^WEBVTT\s*/, "")
		.trim()
		.split(/\n\n+/);
	return cues
		.map((cue, i) => `${i + 1}\n${cue.replace(/\./g, ",")}`)
		.join("\n\n")
		.concat("\n");
}
