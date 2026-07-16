<script lang="ts">
	import { formatBytes, formatDate, formatDuration } from "$lib/dashboard/format";
	import * as DropdownMenu from "@recast/ui/dropdown-menu";
	import { Archive, Clock, Film, MoreHorizontal, Trash2, TriangleAlert } from "@recast/icons";

	export type ArchivedRecast = {
		id: string;
		title: string;
		durationSec: number;
		sizeBytes: number;
		posterUrl: string | null;
		archivedAt: number;
		deletesAt: number;
	};

	let {
		recast,
		ondelete,
	}: {
		recast: ArchivedRecast;
		ondelete: () => void;
	} = $props();

	let posterFailed = $state(false);
	const showPoster = $derived(!!recast.posterUrl && !posterFailed);

	// Whole days until the hard-delete sweep purges this row. Clamped at 0 —
	// a row past its window is just awaiting the next sweep.
	const daysLeft = $derived(
		Math.max(0, Math.ceil((recast.deletesAt - Date.now()) / 86_400_000)),
	);
	const urgent = $derived(daysLeft <= 3);
</script>

<article
	class="glass-card group/card relative flex h-full flex-col overflow-hidden rounded-xl"
>
	<!-- Thumbnail — desaturated; the blob is gone so there's nothing to play. -->
	<div class="relative h-44 w-full shrink-0 overflow-hidden bg-foreground/5">
		{#if showPoster}
			<img
				src={recast.posterUrl}
				alt=""
				loading="lazy"
				onerror={() => (posterFailed = true)}
				class="absolute inset-0 h-full w-full object-cover opacity-40 grayscale"
			/>
		{:else}
			<div
				aria-hidden="true"
				class="absolute inset-0 opacity-50"
				style="background-image: radial-gradient(circle, color-mix(in srgb, var(--color-foreground) 8%, transparent) 1px, transparent 1px); background-size: 16px 16px;"
			></div>
		{/if}

		<!-- Archived overlay -->
		<div class="absolute inset-0 grid place-items-center bg-background/45 backdrop-blur-[1px]">
			<span class="grid size-12 place-items-center rounded-xl border border-border-low/60 bg-background/70 text-muted-foreground shadow-craft-sm">
				{#if showPoster}
					<Archive class="size-5" />
				{:else}
					<Film class="size-5" />
				{/if}
			</span>
		</div>

		<span class="absolute bottom-2.5 right-2.5 z-20 flex items-center gap-1 rounded-md bg-background/85 px-1.5 py-0.5 font-mono text-[10px] font-semibold tabular-nums text-foreground ring-1 ring-inset ring-border-low/50 backdrop-blur-sm">
			<Clock class="size-3" />
			{formatDuration(recast.durationSec)}
		</span>

		<span class="absolute left-2.5 top-2.5 z-20 flex items-center gap-1 rounded-md bg-background/85 px-1.5 py-0.5 font-mono text-[10px] font-bold uppercase tracking-wider text-muted-foreground ring-1 ring-inset ring-border-low/50 backdrop-blur-sm">
			<Archive class="size-3" />Archived
		</span>
	</div>

	<!-- Meta -->
	<div class="flex flex-1 flex-col p-4">
		<div class="flex items-start gap-2">
			<div class="min-w-0 flex-1">
				<h3 class="truncate text-sm font-semibold text-foreground" title={recast.title}>
					{recast.title}
				</h3>
				<p class="mt-1 text-xs text-muted-foreground">
					Archived {formatDate(recast.archivedAt)}
				</p>
			</div>

			<DropdownMenu.Root>
				<DropdownMenu.Trigger
					class="grid size-7 shrink-0 place-items-center rounded-md text-muted-foreground outline-none transition-colors hover:bg-foreground/8 hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50"
					aria-label="Archived recast options"
				>
					<MoreHorizontal class="size-4" />
				</DropdownMenu.Trigger>
				<DropdownMenu.Content align="end" sideOffset={6} class="w-52">
					<DropdownMenu.Item
						onclick={ondelete}
						class="text-destructive/90 data-highlighted:text-destructive"
					>
						<Trash2 class="size-4" />
						Delete permanently
					</DropdownMenu.Item>
				</DropdownMenu.Content>
			</DropdownMenu.Root>
		</div>

		<!-- Countdown -->
		<div
			class="mt-3 flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-[11px] font-medium
				{urgent
				? 'bg-destructive/10 text-destructive'
				: 'bg-foreground/5 text-muted-foreground'}"
		>
			<TriangleAlert class="size-3.5 shrink-0" />
			{#if daysLeft === 0}
				Deletes within a day
			{:else}
				Deletes in {daysLeft}{daysLeft === 1 ? " day" : " days"}
			{/if}
			<span class="ml-auto tabular-nums opacity-70">{formatBytes(recast.sizeBytes)}</span>
		</div>

		<p class="mt-2.5 text-[11px] leading-relaxed text-muted-foreground/80">
			The file was removed after 14 days without views. Re-share it from the Recast
			desktop app to restore it.
		</p>
	</div>
</article>
