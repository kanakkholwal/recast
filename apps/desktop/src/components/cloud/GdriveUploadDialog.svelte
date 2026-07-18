<script lang="ts">
	/**
	 * Foreground progress for a Google Drive upload, the Drive counterpart of
	 * CloudShareDialog. Reads live state from the gdrive store (byte progress +
	 * result/error). Minimize keeps the upload running and hands it to the
	 * activity center; the store fires success/error toasts either way. Drive
	 * uploads can be cancelled, so the running state offers Cancel.
	 */
	import { formatSize } from "$lib/format/files";
	import { etaLabel } from "$lib/format/time";
	import { gdrive } from "$lib/stores/gdrive.svelte";
	import {
	  AlertTriangle,
	  Ban,
	  Check,
	  ExternalLink,
	  HardDriveUpload,
	  Link2,
	  Minus,
	} from "@recast/icons";
	import { Button } from "@recast/ui/button";
	import * as Dialog from "@recast/ui/dialog";
	import { Input } from "@recast/ui/input";
	import { toast } from "@recast/ui/sonner";
	import { Spinner } from "@recast/ui/spinner";
	import { cn } from "@recast/ui/utils";

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
		status === "uploading"
			? pct != null
				? `Uploading… ${pct}%`
				: "Starting upload…"
			: title,
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

<Dialog.Root
	open={true}
	onOpenChange={(v) => {
		if (v) return;
		// Backdrop / Esc: background it while running, else dismiss.
		if (status === "uploading") onMinimize();
		else onClose();
	}}
>
	<Dialog.Content showCloseButton={false} class="sm:max-w-lg">
		<Dialog.Header>
			<Dialog.Title class="flex items-center gap-2">
				<span
					class={cn(
						"grid size-7 place-items-center rounded-lg",
						status === "error"
							? "bg-destructive/10 text-destructive"
							: status === "cancelled"
								? "bg-muted text-muted-foreground"
								: "bg-primary/10 text-primary",
					)}
				>
					{#if status === "complete"}
						<Check class="size-3.5" />
					{:else if status === "error"}
						<AlertTriangle class="size-3.5" />
					{:else if status === "cancelled"}
						<Ban class="size-3.5" />
					{:else}
						<HardDriveUpload class="size-3.5" />
					{/if}
				</span>
				{title}
			</Dialog.Title>
			<Dialog.Description class="truncate">{fileName}</Dialog.Description>
		</Dialog.Header>

		{#if status === "complete"}
			<div class="flex items-center gap-2">
				<Input value={link} readonly class="h-9 font-mono text-xs" />
				<Button
					variant="outline"
					size="sm"
					class="h-9 shrink-0 gap-1.5"
					onclick={copyLink}
				>
					<Link2 class="size-3.5" /> Copy
				</Button>
				<Button
					variant="outline"
					size="sm"
					class="h-9 shrink-0 gap-1.5"
					onclick={openLink}
				>
					<ExternalLink class="size-3.5" /> Open
				</Button>
			</div>
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

				{#if status === "uploading"}
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
					<!-- Cancel is a low-emphasis link here, not a footer button, so the
					     destructive action is separated from the primary Minimize and is
					     never the dialog's default focus. -->
					<div class="flex items-center justify-between gap-2">
						<span class="text-[10px] font-medium tabular-nums text-muted-foreground">
							{transferLabel ?? ""}
						</span>
						<button
							type="button"
							class="text-[11px] font-medium text-muted-foreground/80 transition-colors hover:text-foreground hover:underline"
							onclick={() => gdrive.cancelUpload(uploadId)}
						>
							Cancel upload
						</button>
					</div>
				{:else if status === "error"}
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
				<Button onclick={onClose}>
					Done
					<Check />
				</Button>
			{:else}
				<Button variant="ghost"  size="sm" onclick={onClose}>Close</Button>
				<Button size="sm" onclick={() => gdrive.retry(uploadId)}>
					Try again
				</Button>
			{/if}
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
