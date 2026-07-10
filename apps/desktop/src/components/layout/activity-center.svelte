<script lang="ts">
	/**
	 * Titlebar activity center: a bell button that surfaces background upload
	 * progress (Recast Cloud + Google Drive) and their completions in a popover.
	 * Minimizing the share dialog lands here. The button badges the pending count
	 * and the icon pulses while anything is uploading.
	 */
	import { cloudShare } from "$lib/stores/cloudShare.svelte";
	import { gdrive } from "$lib/stores/gdrive.svelte";
	import { cloudPhaseLabel, uploadPct } from "../corner-notifications.logic";
	import { Button } from "@recast/ui/button";
	import * as Popover from "@recast/ui/popover";
	import { toast } from "@recast/ui/sonner";
	import { cn } from "@recast/ui/utils";
	import {
		Bell,
		CheckCircle2,
		Cloud,
		Copy,
		ExternalLink,
		Inbox,
		RefreshCw,
		TriangleAlert,
		X,
	} from "@lucide/svelte";

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
	const total = $derived(cloudItems.length + driveItems.length);
	const busy = $derived(
		cloudItems.some((u) => u.status === "uploading") ||
			driveItems.some((u) => u.status === "uploading"),
	);

	let open = $state(false);

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
				<Bell size={15} class={busy ? "motion-safe:animate-pulse" : ""} />
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
		<div class="border-b border-border/50 px-3 py-2">
			<span
				class="text-[11px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
			>
				Activity
			</span>
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
