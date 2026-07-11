<script lang="ts">
	/**
	 * Titlebar activity center: a bell button that surfaces background upload
	 * progress (Recast Cloud + Google Drive) and their completions in a popover.
	 * Minimizing the share dialog lands here. The button badges the pending count
	 * and the icon pulses while anything is uploading.
	 */
	import { goto } from "$app/navigation";
	import { openFileLocation } from "$lib/ipc";
	import { cloudShare } from "$lib/stores/cloudShare.svelte";
	import {
	  exportActivity,
	  type ExportItem,
	} from "$lib/stores/exportActivity.svelte";
	import { gdrive } from "$lib/stores/gdrive.svelte";
	import {
	  CheckCircle2,
	  Clock,
	  Cloud,
	  Copy,
	  ExternalLink,
	  Film,
	  FolderOpen,
	  Inbox,
	  RefreshCw,
	  TriangleAlert,
	  X
	} from "@lucide/svelte";
	import { Button } from "@recast/ui/button";
	import * as Popover from "@recast/ui/popover";
	import { toast } from "@recast/ui/sonner";
	import { cn } from "@recast/ui/utils";
	import { cloudPhaseLabel, uploadPct } from "../corner-notifications.logic";

	// Cloud uploads shown in a foreground dialog are hidden here; they reappear
	// on minimize.
	const cloudItems = $derived(
		cloudShare.activeUploads.filter(
			(u) => u.sourcePath !== cloudShare.foregroundPath,
		),
	);
	// Same for Drive: hide the one currently in its foreground dialog.
	const driveItems = $derived(
		gdrive.activeUploads.filter((u) => u.uploadId !== gdrive.foregroundId),
	);
	// Export queue, hiding the one item whose panel is on screen in the editor
	// (foregrounded AND an editor mounted); on any other route every item stays
	// visible so a background export is never hidden with nowhere to show it.
	const exportItems = $derived(
		exportActivity.items.filter(
			(it) =>
				!(
					exportActivity.foreground &&
					exportActivity.editorPresent &&
					it.id === exportActivity.foregroundId
				),
		),
	);
	const total = $derived(
		cloudItems.length + driveItems.length + exportItems.length,
	);
	const busy = $derived(
		exportItems.some((i) => i.status === "running") ||
			cloudItems.some((u) => u.status === "uploading") ||
			driveItems.some((u) => u.status === "uploading"),
	);

	const exportPhaseLabel: Record<string, string> = {
		preparing: "Preparing export",
		encoding: "Rendering video",
		finalizing: "Finalising file",
		cancelling: "Cancelling export",
	};

	// "Clear all" dismisses every FINISHED item across the panel, leaving anything
	// still in progress or queued/uploading. Reuses each store's per-item dismiss.
	const clearableExports = $derived(
		exportItems.filter((i) => i.status !== "running" && i.status !== "queued"),
	);
	const clearableCloud = $derived(
		cloudItems.filter((u) => u.status !== "uploading"),
	);
	const clearableDrive = $derived(
		driveItems.filter((u) => u.status !== "uploading"),
	);
	const clearableCount = $derived(
		clearableExports.length + clearableCloud.length + clearableDrive.length,
	);

	function clearAll() {
		for (const it of clearableExports) exportActivity.dismiss(it.id);
		for (const u of clearableCloud) cloudShare.dismiss(u.sourcePath);
		for (const u of clearableDrive) gdrive.dismissUpload(u.uploadId);
	}

	let open = $state(false);

	// A finished export opens the Exports page; an active/queued one reopens its
	// panel in the owning editor.
	function openExportItem(item: ExportItem) {
		open = false;
		if (item.status === "success") {
			void goto("/exports");
			return;
		}
		exportActivity.show(item.id);
	}

	async function showExportInFolder(path: string) {
		try {
			await openFileLocation(path);
		} catch (e) {
			toast.error(`Could not open folder: ${e}`);
		}
	}

	// Reopen the foreground share dialog for a Recast Cloud upload: progress while
	// it runs, share settings once it lands. Closes the popover so the two
	// overlays don't fight.
	function openShare(path: string) {
		open = false;
		cloudShare.setForeground(path);
	}

	// Same for a Google Drive upload: reopen its progress/result dialog.
	function openDrive(uploadId: string) {
		open = false;
		gdrive.setForeground(uploadId);
	}

	async function copy(link: string, label: string) {
		try {
			await navigator.clipboard.writeText(link);
			toast.success(label);
		} catch (e) {
			toast.error(`Could not copy link: ${e}`);
		}
	}
	async function openLink(link: string) {
		try {
			const { openUrl } = await import("@tauri-apps/plugin-opener");
			await openUrl(link);
		} catch {
			window.open(link, "_blank", "noopener");
		}
	}
</script>

<Popover.Root {open} onOpenChange={(v) => (open = v)}>
	<Popover.Trigger>
		{#snippet child({ props })}
			<button
				{...props as Record<string, unknown>}
				type="button"
				onmousedown={(e) => e.stopPropagation()}
				aria-label={total > 0 ? `Activity, ${total} item${total === 1 ? "" : "s"}` : "Activity"}
				title="Activity"
				class="group relative inline-flex size-7 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground data-[state=open]:bg-card data-[state=open]:text-foreground"
			>
				<Inbox size={15} class={busy ? "motion-safe:animate-pulse" : ""} />
				{#if total > 0}
					<span
						class="absolute -right-0.5 -top-0.5 grid h-3.5 min-w-3.5 place-items-center rounded-full bg-primary px-1 text-[9px] font-bold leading-none text-primary-foreground"
					>
						{total}
					</span>
				{/if}
			</button>
		{/snippet}
	</Popover.Trigger>
	<Popover.Content align="end" sideOffset={8} class="w-80 p-0">
		<div
			class="flex items-center justify-between gap-2 border-b border-border/50 px-3 py-2"
		>
			<span
				class="text-[11px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
			>
				Activity
			</span>
			{#if clearableCount > 0}
				<button
					type="button"
					onclick={clearAll}
					class="cursor-pointer rounded text-[10.5px] font-medium text-muted-foreground/70 transition-colors hover:text-foreground focus-visible:text-foreground focus-visible:outline-none"
				>
					Clear all
				</button>
			{/if}
		</div>

		{#if total === 0}
			<div class="flex flex-col items-center gap-2 px-4 py-8 text-center">
				<span
					class="grid size-9 place-items-center rounded-lg bg-foreground/5 text-muted-foreground/60"
				>
					<Inbox class="size-4" />
				</span>
				<p class="text-[11.5px] font-medium text-foreground">No activity</p>
				<p class="text-[10.5px] text-muted-foreground/70">
					Uploads and shares show up here.
				</p>
			</div>
		{:else}
			<div
				class="flex max-h-[min(70vh,420px)] flex-col divide-y divide-border/40 overflow-y-auto"
			>
				<!-- Export queue. Clicking a running/queued item reopens its editor
				     panel; a finished one opens the Exports page. Action buttons are
				     siblings so nothing interactive is nested. -->
				{#each exportItems as item (item.id)}
					<div class="flex flex-col gap-2 px-3 py-2.5">
						<div class="flex items-start gap-2.5">
							<button
								type="button"
								title={item.status === "success" ? "Open in Exports" : "Open export"}
								onclick={() => openExportItem(item)}
								class="-my-1 flex min-w-0 flex-1 items-start gap-2.5 rounded-md py-1 text-left outline-none transition-colors hover:bg-foreground/3 focus-visible:bg-foreground/3"
							>
								<span
									class={cn(
										"grid size-7 shrink-0 place-items-center rounded-lg",
										item.status === "error" || item.status === "interrupted"
											? "bg-destructive/10 text-destructive"
											: item.status === "queued"
												? "bg-muted text-muted-foreground"
												: "bg-primary/10 text-primary",
									)}
								>
									{#if item.status === "running"}
										<Film class="size-3.5 motion-safe:animate-pulse" />
									{:else if item.status === "queued"}
										<Clock class="size-3.5" />
									{:else if item.status === "success"}
										<CheckCircle2 class="size-3.5" />
									{:else if item.status === "cancelled"}
										<X class="size-3.5" />
									{:else}
										<TriangleAlert class="size-3.5" />
									{/if}
								</span>
								<div class="min-w-0 flex-1">
									<p class="text-[12px] font-semibold leading-tight text-foreground">
										{#if item.status === "running"}
											{exportPhaseLabel[item.phase]}
										{:else if item.status === "queued"}
											Queued
										{:else if item.status === "success"}
											Export complete
										{:else if item.status === "cancelled"}
											Export cancelled
										{:else if item.status === "interrupted"}
											Export interrupted
										{:else}
											Export failed
										{/if}
									</p>
									<p
										class="mt-0.5 truncate text-[11px] text-muted-foreground"
										title={item.filename}
									>
										{(item.status === "error" || item.status === "interrupted") &&
										item.error
											? item.error
											: item.filename}
									</p>
								</div>
							</button>
							<button
								type="button"
								class="-mr-1 -mt-0.5 shrink-0 rounded-md p-1 text-muted-foreground/60 transition-colors hover:bg-foreground/5 hover:text-foreground disabled:pointer-events-none disabled:opacity-40"
								aria-label={item.status === "queued" ? "Remove from queue" : "Dismiss"}
								disabled={item.status === "running"}
								onclick={() =>
									item.status === "queued"
										? exportActivity.cancel(item.id)
										: exportActivity.dismiss(item.id)}
							>
								<X class="size-3.5" />
							</button>
						</div>
						{#if item.status === "running"}
							<div class="h-1 overflow-hidden rounded-full bg-muted">
								{#if item.phase === "encoding"}
									<div
										class="h-full rounded-full bg-primary transition-[width] duration-200"
										style="width: {Math.round(item.progress)}%"
									></div>
								{:else}
									<div
										class="h-full w-1/3 rounded-full bg-primary motion-safe:animate-pulse"
									></div>
								{/if}
							</div>
							<div class="flex items-center justify-between">
								<span
									class="font-mono text-[11px] font-semibold tabular-nums text-primary"
								>
									{item.phase === "encoding" || item.phase === "finalizing"
										? `${Math.round(item.progress)}%`
										: item.phase === "preparing"
											? "Preparing…"
											: ""}
								</span>
								<button
									type="button"
									class="text-[10px] font-medium text-muted-foreground/80 transition-colors hover:text-foreground hover:underline disabled:pointer-events-none disabled:opacity-50"
									disabled={item.phase === "cancelling"}
									onclick={() => exportActivity.cancel(item.id)}
								>
									{item.phase === "cancelling" ? "Cancelling…" : "Cancel"}
								</button>
							</div>
						{:else if item.status === "success" && item.path}
							<div class="flex items-center justify-end gap-1.5">
								<Button
									size="xs"
									variant="ghost"
									class="h-7 gap-1.5"
									onclick={() => showExportInFolder(item.path!)}
								>
									<FolderOpen class="size-3" /> Show in folder
								</Button>
								<Button
									size="xs"
									class="h-7 gap-1.5"
									onclick={() => openExportItem(item)}
								>
									<ExternalLink class="size-3" /> Exports
								</Button>
							</div>
						{:else if item.status === "error" || item.status === "cancelled" || item.status === "interrupted"}
							<div class="flex items-center justify-end">
								<Button
									size="xs"
									variant="ghost"
									class="h-7 gap-1.5"
									onclick={() => exportActivity.retry(item.id)}
								>
									<RefreshCw class="size-3" /> Retry
								</Button>
							</div>
						{/if}
					</div>
				{/each}

				<!-- Recast Cloud shares. The info area is the click target that reopens
				     the dialog; the action buttons are siblings, so no interactive
				     control is nested inside another. -->
				{#each cloudItems as up (up.sourcePath)}
					<div class="flex flex-col gap-2 px-3 py-2.5">
						<div class="flex items-start gap-2.5">
							<button
								type="button"
								title="Open share"
								onclick={() => openShare(up.sourcePath)}
								class="-my-1 flex min-w-0 flex-1 items-start gap-2.5 rounded-md py-1 text-left outline-none transition-colors hover:bg-foreground/3 focus-visible:bg-foreground/3"
							>
								<span
									class={cn(
										"grid size-7 shrink-0 place-items-center rounded-lg",
										up.status === "error"
											? "bg-destructive/10 text-destructive"
											: "bg-primary/10 text-primary",
									)}
								>
									{#if up.status === "uploading"}
										<Cloud class="size-3.5 motion-safe:animate-pulse" />
									{:else if up.status === "complete"}
										<CheckCircle2 class="size-3.5" />
									{:else}
										<TriangleAlert class="size-3.5" />
									{/if}
								</span>
								<div class="min-w-0 flex-1">
									<p class="text-[12px] font-semibold leading-tight text-foreground">
										{#if up.status === "uploading"}
											{cloudPhaseLabel(up.phase)}
										{:else if up.status === "complete"}
											Shared to Recast Cloud
										{:else}
											Share failed
										{/if}
									</p>
									<p
										class="mt-0.5 truncate text-[11px] text-muted-foreground"
										title={up.fileName}
									>
										{up.status === "error" && up.error ? up.error : up.fileName}
									</p>
								</div>
							</button>
							<button
								type="button"
								class="-mr-1 -mt-0.5 shrink-0 rounded-md p-1 text-muted-foreground/60 transition-colors hover:bg-foreground/5 hover:text-foreground"
								aria-label="Dismiss"
								onclick={() => cloudShare.dismiss(up.sourcePath)}
							>
								<X class="size-3.5" />
							</button>
						</div>
						{#if up.status === "uploading"}
							<div class="h-1 overflow-hidden rounded-full bg-muted">
								{#if up.phase === "uploading" && up.totalBytes > 0}
									<div
										class="h-full rounded-full bg-primary transition-[width] duration-200"
										style="width: {uploadPct(up.bytesSent, up.totalBytes)}%"
									></div>
								{:else}
									<div
										class="h-full w-1/3 rounded-full bg-primary motion-safe:animate-pulse"
									></div>
								{/if}
							</div>
						{:else if up.status === "complete" && up.shareUrl}
							<div class="flex items-center justify-end gap-1.5">
								<Button
									size="xs"
									variant="ghost"
									class="h-7 gap-1.5"
									onclick={() => copy(up.shareUrl!, "Share link copied.")}
								>
									<Copy class="size-3" /> Copy link
								</Button>
								<Button
									size="xs"
									class="h-7 gap-1.5"
									onclick={() => openLink(up.shareUrl!)}
								>
									<ExternalLink class="size-3" /> Open
								</Button>
							</div>
						{/if}
					</div>
				{/each}

				<!-- Google Drive uploads. The info area is the click target that reopens
				     the dialog; the action buttons are siblings, so no interactive
				     control is nested inside another. -->
				{#each driveItems as up (up.uploadId)}
					<div class="flex flex-col gap-2 px-3 py-2.5">
						<div class="flex items-start gap-2.5">
							<button
								type="button"
								title="Open upload"
								onclick={() => openDrive(up.uploadId)}
								class="-my-1 flex min-w-0 flex-1 items-start gap-2.5 rounded-md py-1 text-left outline-none transition-colors hover:bg-foreground/3 focus-visible:bg-foreground/3"
							>
								<span
									class={cn(
										"grid size-7 shrink-0 place-items-center rounded-lg",
										up.status === "error"
											? "bg-destructive/10 text-destructive"
											: "bg-primary/10 text-primary",
									)}
								>
									{#if up.status === "uploading"}
										<RefreshCw class="size-3.5 motion-safe:animate-spin" />
									{:else if up.status === "complete"}
										<CheckCircle2 class="size-3.5" />
									{:else}
										<TriangleAlert class="size-3.5" />
									{/if}
								</span>
								<div class="min-w-0 flex-1">
									<p class="text-[12px] font-semibold leading-tight text-foreground">
										{#if up.status === "uploading"}
											Uploading to Drive
										{:else if up.status === "complete"}
											Uploaded to Drive
										{:else if up.status === "cancelled"}
											Upload cancelled
										{:else}
											Upload failed
										{/if}
									</p>
									<p
										class="mt-0.5 truncate text-[11px] text-muted-foreground"
										title={up.fileName}
									>
										{up.status === "error" && up.error ? up.error : up.fileName}
									</p>
								</div>
							</button>
							<button
								type="button"
								class="-mr-1 -mt-0.5 shrink-0 rounded-md p-1 text-muted-foreground/60 transition-colors hover:bg-foreground/5 hover:text-foreground"
								aria-label="Dismiss"
								onclick={() => gdrive.dismissUpload(up.uploadId)}
							>
								<X class="size-3.5" />
							</button>
						</div>
						{#if up.status === "uploading"}
							<div class="h-1 overflow-hidden rounded-full bg-muted">
								<div
									class="h-full rounded-full bg-primary transition-[width] duration-200"
									style="width: {uploadPct(up.bytesSent, up.totalBytes)}%"
								></div>
							</div>
							<div class="flex items-center justify-between">
								<span
									class="text-[10px] font-medium tabular-nums text-muted-foreground"
								>
									{uploadPct(up.bytesSent, up.totalBytes)}%
								</span>
								<button
									type="button"
									class="text-[10px] font-medium text-muted-foreground/80 transition-colors hover:text-foreground hover:underline"
									onclick={() => gdrive.cancelUpload(up.uploadId)}
								>
									Cancel
								</button>
							</div>
						{:else if up.status === "complete" && up.webViewLink}
							<div class="flex items-center justify-end gap-1.5">
								<Button
									size="xs"
									variant="ghost"
									class="h-7 gap-1.5"
									onclick={() => copy(up.webViewLink!, "Drive link copied.")}
								>
									<Copy class="size-3" /> Copy link
								</Button>
								<Button
									size="xs"
									class="h-7 gap-1.5"
									onclick={() => openLink(up.webViewLink!)}
								>
									<ExternalLink class="size-3" /> Open in Drive
								</Button>
							</div>
						{/if}
					</div>
				{/each}
			</div>
		{/if}
	</Popover.Content>
</Popover.Root>
