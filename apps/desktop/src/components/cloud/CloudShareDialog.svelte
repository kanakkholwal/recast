<script lang="ts">
/**
 * Foreground progress for a Recast Cloud share. Reads live upload state from
 * the cloudShare store (phase + byte counts + result/error). When the upload
 * finishes it shows the share settings inline (link, visibility, password,
 * expiry) like the web QuickUpload flow. Minimize keeps the upload running and
 * hands it to the activity center. The store also fires success/error toasts,
 * so feedback still lands if this dialog is minimized.
 */

import DialogShell from "@recast/editor/components/dialog/DialogShell.svelte";
import { formatSize } from "@recast/editor/lib/format/files";
import { etaLabel } from "@recast/editor/lib/format/time";
import { AlertTriangle, Check, Minus } from "@recast/icons";
import { Button } from "@recast/ui/button";
import Logo from "$components/logo.svelte";
import UploadProgress from "$components/recast/UploadProgress.svelte";
import { cloudShare } from "$lib/stores/cloudShare.svelte";
import CloudShareSettings from "./CloudShareSettings.svelte";

let { path }: { path: string } = $props();

const upload = $derived(cloudShare.uploads[path]);
const record = $derived(cloudShare.uploadHistory[path]);
const fileName = $derived(upload?.fileName ?? path.split(/[\\/]/).pop() ?? "");
const status = $derived(upload?.status ?? "uploading");

/** Background the upload and dismiss the dialog (upload keeps running, and
 * resurfaces in the activity center). */
function onMinimize() {
	cloudShare.setForeground(null);
}
/** Terminal-state dismiss: clears the store entry and closes. */
function onClose() {
	cloudShare.setForeground(null);
	cloudShare.dismiss(path);
}
function onRetry() {
	cloudShare.retry(path);
}
const phase = $derived(upload?.phase ?? "preparing");
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

const phaseLabel = $derived(
	status === "error"
		? "Upload failed"
		: phase === "preparing"
			? "Preparing…"
			: phase === "uploading"
				? pct != null
					? `Uploading… ${pct}%`
					: "Uploading…"
				: phase === "finalizing"
					? "Finalizing…"
					: "Creating share link…",
);

// Determinate only during the byte upload; other phases sweep indeterminately.
const indeterminate = $derived(status === "uploading" && pct == null);

let save = $state<() => Promise<boolean>>(async () => true);
let saving = $state(false);
let loading = $state(true);

async function done() {
	if (await save()) onClose();
}
</script>

<DialogShell
	open={true}
	title={status === "complete" ? "Shared to Recast Cloud" : "Share to Recast Cloud"}
	subtitle={fileName}
	icon={status === "complete" ? Check : status === "error" ? AlertTriangle : Logo}
	tone={status === "error" ? "destructive" : "default"}
	widthClass="sm:max-w-lg"
	onOpenChange={(v) => {
		if (v) return;
		// Backdrop / Esc: background it while still uploading, else dismiss.
		if (status === "uploading") onMinimize();
		else onClose();
	}}
>
	{#if status === "complete" && record}
		<CloudShareSettings
			recastId={record.recastId}
			slug={record.slug}
			shareUrl={record.shareUrl}
			bind:save
			bind:saving
			bind:loading
		/>
	{:else}
		<UploadProgress
			{phaseLabel}
			{pct}
			active={status === "uploading"}
			failed={status === "error"}
			{transferLabel}
		/>
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
			<Button size="xs" disabled={saving || loading} onclick={done}>
				{saving ? "Saving…" : "Done"}
			</Button>
		{:else}
			<Button variant="ghost" size="xs" onclick={onClose}>Close</Button>
			<Button size="xs" onclick={onRetry}>Try again</Button>
		{/if}
	{/snippet}
</DialogShell>
