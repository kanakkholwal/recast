<script lang="ts">
/**
 * Foreground progress for a Google Drive upload, the Drive counterpart of
 * CloudShareDialog. Reads live state from the gdrive store (byte progress +
 * result/error). Minimize keeps the upload running and hands it to the
 * activity center; the store fires success/error toasts either way. Drive
 * uploads can be cancelled, so the running state offers Cancel.
 */

import DialogShell from "@recast/editor/components/dialog/DialogShell.svelte";
import { formatSize } from "@recast/editor/lib/format/files";
import { etaLabel } from "@recast/editor/lib/format/time";
import {
	AlertTriangle,
	Ban,
	BrandGoogleDrive,
	Check,
	ExternalLink,
	Link2,
	Minus,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import { Input } from "@recast/ui/input";
import { toast } from "@recast/ui/sonner";
import UploadProgress from "$components/recast/UploadProgress.svelte";
import { gdrive } from "$lib/stores/gdrive.svelte";

let { uploadId }: { uploadId: string } = $props();

const upload = $derived(gdrive.uploads[uploadId]);
const fileName = $derived(upload?.fileName ?? "");
const status = $derived(upload?.status ?? "uploading");
const link = $derived(upload?.webViewLink ?? "");
const pct = $derived(
	upload && upload.totalBytes > 0
		? Math.min(100, Math.round((upload.bytesSent / upload.totalBytes) * 100))
		: null,
);
// Byte and ETA readout, so a multi-minute upload feels in-control rather than stalled.
const transferLabel = $derived.by(() => {
	if (!upload || upload.totalBytes <= 0) return null;
	const size = `${formatSize(upload.bytesSent)} of ${formatSize(upload.totalBytes)}`;
	const eta =
		upload.bytesPerSec && upload.bytesPerSec > 0
			? etaLabel((upload.totalBytes - upload.bytesSent) / upload.bytesPerSec)
			: null;
	return eta ? `${size} · ${eta}` : size;
});
// Determinate once bytes are flowing; a short indeterminate sweep before then.
const indeterminate = $derived(status === "uploading" && pct == null);

const title = $derived(
	status === "complete"
		? "Uploaded to Google Drive"
		: status === "error"
			? "Upload failed"
			: status === "cancelled"
				? "Upload cancelled"
				: "Uploading to Google Drive",
);
const phaseLabel = $derived(
	status === "uploading" ? (pct != null ? `Uploading… ${pct}%` : "Starting upload…") : title,
);

function onMinimize() {
	gdrive.setForeground(null);
}
function onClose() {
	gdrive.setForeground(null);
	gdrive.dismissUpload(uploadId);
}

async function copyLink() {
	try {
		await navigator.clipboard.writeText(link);
		toast.success("Drive link copied.");
	} catch (e) {
		toast.error(`Couldn't copy: ${e}`);
	}
}
async function openLink() {
	try {
		const { openUrl } = await import("@tauri-apps/plugin-opener");
		await openUrl(link);
	} catch {
		window.open(link, "_blank", "noopener");
	}
}
</script>

<DialogShell
	open={true}
	{title}
	subtitle={fileName}
	icon={status === "complete"
		? Check
		: status === "error"
			? AlertTriangle
			: status === "cancelled"
				? Ban
				: BrandGoogleDrive}
	tone={status === "error" ? "destructive" : status === "cancelled" ? "muted" : "default"}
	widthClass="sm:max-w-lg"
	onOpenChange={(v) => {
		if (v) return;
		if (status === "uploading") onMinimize();
		else onClose();
	}}
>
	{#if status === "complete"}
		<div class="flex items-center gap-2">
			<Input value={link} readonly aria-label="Drive link" class="h-9 font-mono text-xs" />
			<Button variant="outline" size="sm" class="h-9 shrink-0 gap-1.5" onclick={copyLink}>
				<Link2 class="size-3.5" /> Copy
			</Button>
			<Button variant="outline" size="sm" class="h-9 shrink-0 gap-1.5" onclick={openLink}>
				<ExternalLink class="size-3.5" /> Open
			</Button>
		</div>
	{:else}
		<UploadProgress
			{phaseLabel}
			{pct}
			active={status === "uploading"}
			failed={status === "error"}
			{transferLabel}
		>
			{#snippet trailing()}
				<!-- Cancel is a low-emphasis link, not a footer button, so the
				     destructive action is never the dialog's default focus. -->
				{#if status === "uploading"}
					<button
						type="button"
						class="text-[11px] font-medium text-muted-foreground/80 transition-colors hover:text-foreground hover:underline"
						onclick={() => gdrive.cancelUpload(uploadId)}
					>
						Cancel upload
					</button>
				{/if}
			{/snippet}
		</UploadProgress>
		{#if status === "error"}
			<p class="mt-2.5 text-[11px] leading-relaxed text-muted-foreground">
				{upload?.error ?? "Something went wrong during the upload."}
			</p>
		{/if}
	{/if}

	{#snippet footer()}
		{#if status === "uploading"}
			<Button variant="default_soft" size="xs" onclick={onMinimize}>
				<Minus />
				Minimize
			</Button>
		{:else if status === "complete"}
			<Button size="xs" onclick={onClose}>Done</Button>
		{:else}
			<Button variant="ghost" size="xs" onclick={onClose}>Close</Button>
			<Button size="xs" onclick={() => gdrive.retry(uploadId)}>Try again</Button>
		{/if}
	{/snippet}
</DialogShell>
