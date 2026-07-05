/**
 * Shared upload controller — the reactive state + orchestration behind the
 * dashboard's "Upload" flow, used identically by the Home and Library pages
 * (header button, empty-state button, file input, drag-and-drop). Holds the
 * in-flight `$state` so both routes get the same progress UI without copying
 * the whole start/finalize/copy-link dance.
 */

import { toast } from "@recast/ui/sonner";
import { uploadPhaseLabel, uploadRecastFile, type UploadPhase } from "./upload";

interface UploadControllerOptions {
	/** Reactive workspace id — read fresh at upload time. */
	workspaceId: () => string | undefined;
	/** Re-run the page loaders after a successful upload (e.g. invalidateAll). */
	onRefresh: () => Promise<void>;
}

export function createUploadController(options: UploadControllerOptions) {
	let uploading = $state(false);
	let phase = $state<UploadPhase>("preparing");
	let pct = $state(0);

	async function start(file: File) {
		if (uploading) return;
		uploading = true;
		phase = "preparing";
		pct = 0;
		try {
			const result = await uploadRecastFile(file, {
				workspaceId: options.workspaceId(),
				onPhase: (p) => (phase = p),
				onProgress: (v) => (pct = v),
			});
			await options.onRefresh();
			let copied = false;
			try {
				await navigator.clipboard.writeText(result.shareUrl);
				copied = true;
			} catch {
				copied = false;
			}
			toast.success(
				copied
					? `“${file.name}” uploaded. Share link copied to clipboard.`
					: `“${file.name}” uploaded and shared.`,
			);
		} catch (err) {
			toast.error((err as Error)?.message ?? "Couldn't upload that file.");
		} finally {
			uploading = false;
		}
	}

	/** Consume a file input's change event: take the first file, reset the input
	 *  so the same file can be re-picked, and kick off the upload. */
	function onFilePicked(e: Event) {
		const input = e.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		input.value = "";
		if (file) start(file);
	}

	return {
		get uploading() {
			return uploading;
		},
		get phase() {
			return phase;
		},
		get pct() {
			return pct;
		},
		get label() {
			return uploadPhaseLabel(phase, pct);
		},
		start,
		onFilePicked,
	};
}
