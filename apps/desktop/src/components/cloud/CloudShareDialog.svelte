<script lang="ts">
	/**
	 * Foreground progress for a Recast Cloud share. Reads live upload state from
	 * the cloudShare store (phase + byte counts + result/error). When the upload
	 * finishes it shows the share settings inline (link, visibility, password,
	 * expiry) like the web QuickUpload flow. Minimize keeps the upload running and
	 * hands it to the activity center. The store also fires success/error toasts,
	 * so feedback still lands if this dialog is minimized.
	 */
	import { formatSize } from "$lib/format/files";
	import { etaLabel } from "$lib/format/time";
	import { cloudShare } from "$lib/stores/cloudShare.svelte";
	import { AlertTriangle, Check, Cloud, Minus } from "@lucide/svelte";
	import { Button } from "@recast/ui/button";
	import * as Dialog from "@recast/ui/dialog";
	import { Spinner } from "@recast/ui/spinner";
	import { cn } from "@recast/ui/utils";
	import CloudShareSettings from "./CloudShareSettings.svelte";

	let { path }: { path: string } = $props();

	const upload = $derived(cloudShare.uploads[path]);
	const record = $derived(cloudShare.uploadHistory[path]);
	const fileName = $derived(
		upload?.fileName ?? path.split(/[\\/]/).pop() ?? "",
	);
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
	// Byte + ETA readout during the transfer so a multi-minute upload feels
	// in-control (e.g. "12.3 MB of 45.0 MB · ~40s left").
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

<Dialog.Root
	open={true}
	onOpenChange={(v) => {
		if (v) return;
		// Backdrop / Esc: background it while still uploading, else dismiss.
		if (status === "uploading") onMinimize();
		else onClose();
	}}
>
	<Dialog.Content
		showCloseButton={false}
		class="max-h-[min(88vh,720px)] overflow-y-auto sm:max-w-lg"
	>
		<Dialog.Header>
			<Dialog.Title class="flex items-center gap-2">
				<span
					class={cn(
						"grid size-7 place-items-center rounded-lg",
						status === "error"
							? "bg-destructive/10 text-destructive"
							: "bg-primary/10 text-primary",
					)}
				>
					{#if status === "complete"}
						<Check class="size-3.5" />
					{:else if status === "error"}
						<AlertTriangle class="size-3.5" />
					{:else}
						<Cloud class="size-3.5" />
					{/if}
				</span>
				{status === "complete" ? "Shared to Recast Cloud" : "Share to Recast Cloud"}
			</Dialog.Title>
			<Dialog.Description class="truncate">{fileName}</Dialog.Description>
		</Dialog.Header>

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
			<div class="space-y-2.5" aria-live="polite">
				<div class="flex items-center justify-between gap-2 text-xs">
					<span
						class={cn(
							"font-medium",
							status === "error" ? "text-destructive" : "text-foreground",
						)}
					>
						{phaseLabel}
					</span>
					{#if status === "uploading"}
						<Spinner class="size-3.5 shrink-0 text-muted-foreground" />
					{/if}
				</div>

				{#if status !== "error"}
					<div class="h-1.5 w-full overflow-hidden rounded-full bg-muted">
						{#if indeterminate}
							<div
								class="h-full w-1/3 rounded-full bg-primary motion-safe:animate-pulse"
							></div>
						{:else}
							<div
								class="h-full rounded-full bg-primary transition-[width] duration-200"
								style="width: {pct ?? 0}%"
							></div>
						{/if}
					</div>
					{#if transferLabel}
						<p class="text-[10px] font-medium tabular-nums text-muted-foreground">
							{transferLabel}
						</p>
					{/if}
				{/if}

				{#if status === "error"}
					<p class="text-[11px] leading-relaxed text-muted-foreground">
						{upload?.error ?? "Something went wrong during the upload."}
					</p>
				{/if}
			</div>
		{/if}

		<Dialog.Footer>
			{#if status === "uploading"}
				<Button variant="default_soft" size="sm" onclick={onMinimize}>
					<Minus />
					Minimize
				</Button>
			{:else if status === "complete"}
				<Button disabled={saving || loading}  size="sm" onclick={done}>
					{saving ? "Saving…" : "Done"}
					{#if !saving}<Check />{/if}
				</Button>
			{:else}
				<Button variant="ghost"  size="sm" onclick={onClose}>Close</Button>
				<Button onclick={onRetry}>Try again</Button>
			{/if}
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
