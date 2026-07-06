/**
 * Shared upload controller: the reactive state and orchestration behind the
 * dashboard upload flows. Used by home, library, and the global quick-upload
 * dialog so progress, refresh, share creation, and clipboard behavior stay in
 * one place.
 */

import { toast } from "@recast/ui/sonner";
import {
	uploadPhaseLabel,
	uploadRecastFile,
	type UploadPhase,
	type UploadResult,
	type ShareOptions,
} from "./upload";

interface UploadControllerOptions {
	/** Reactive workspace id, read fresh at upload time. */
	workspaceId: () => string | undefined;
	share?: () => ShareOptions | undefined;
	/** Re-run page loaders after a successful upload. */
	onRefresh: () => Promise<void>;
}

export function createUploadController(options: UploadControllerOptions) {
	let uploading = $state(false);
	let phase = $state<UploadPhase>("preparing");
	let pct = $state(0);
	let lastResult = $state<UploadResult | null>(null);
	let lastFileName = $state("");

	async function start(file: File): Promise<UploadResult | null> {
		if (uploading) return null;
		uploading = true;
		phase = "preparing";
		pct = 0;
		lastResult = null;
		lastFileName = file.name;

		try {
			const result = await uploadRecastFile(file, {
				workspaceId: options.workspaceId(),
				share: options.share?.(),
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
					? `"${file.name}" uploaded. Share link copied to clipboard.`
					: `"${file.name}" uploaded and shared.`,
			);
			lastResult = result;
			return result;
		} catch (err) {
			toast.error((err as Error)?.message ?? "Couldn't upload that file.");
			return null;
		} finally {
			uploading = false;
		}
	}

	function onFilePicked(e: Event) {
		const input = e.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		input.value = "";
		if (file) void start(file);
	}

	function resetResult() {
		lastResult = null;
		lastFileName = "";
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
		get lastResult() {
			return lastResult;
		},
		get lastFileName() {
			return lastFileName;
		},
		start,
		onFilePicked,
		resetResult,
	};
}
