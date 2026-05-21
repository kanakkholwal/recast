<script lang="ts">
	import { formatCount, formatDuration } from "$lib/dashboard/format";
	import type { Recast } from "$lib/dashboard/store.svelte";
	import { Cloud, Crown, Film } from "@lucide/svelte";

	let {
		recasts,
		limit = 3,
	}: {
		recasts: Recast[];
		limit?: number;
	} = $props();

	const items = $derived(
		[...recasts]
			.filter((r) => r.source === "cloud")
			.sort((a, b) => b.views - a.views)
			.slice(0, limit),
	);
</script>

<section class="glass-card flex h-full flex-col rounded-xl">
	<header class="flex items-center justify-between border-b border-border-low/50 px-5 py-3.5">
		<div class="flex items-center gap-2">
			<Crown class="size-4 text-primary" />
			<h2 class="text-sm font-semibold text-foreground">Top recasts</h2>
		</div>
		<a
			href="/dashboard/recasts"
			class="text-[11px] font-semibold uppercase tracking-[0.14em] text-muted-foreground transition-colors hover:text-foreground"
		>
			View all →
		</a>
	</header>

	{#if items.length === 0}
		<div class="flex flex-1 flex-col items-center justify-center px-5 py-10 text-center">
			<span class="glass-chip grid size-10 place-items-center rounded-lg text-muted-foreground">
				<Cloud class="size-4" />
			</span>
			<p class="mt-3 text-xs text-muted-foreground">
				Share a recast to start gathering views.
			</p>
		</div>
	{:else}
		<ol class="divide-y divide-border-low/40">
			{#each items as rec, i (rec.id)}
				<li class="flex items-center gap-3 px-5 py-3">
					<span class="grid size-6 shrink-0 place-items-center rounded-md bg-foreground/5 font-mono text-[11px] font-bold text-foreground/70">
						{i + 1}
					</span>
					<div class="min-w-0 flex-1">
						<p class="truncate text-sm font-semibold text-foreground" title={rec.title}>
							{rec.title}
						</p>
						<p class="mt-0.5 flex items-center gap-1.5 text-[11px] text-muted-foreground">
							<Film class="size-3" />
							{formatDuration(rec.durationSec)}
						</p>
					</div>
					<div class="text-right">
						<div class="font-mono text-sm font-semibold tabular-nums text-foreground">
							{formatCount(rec.views)}
						</div>
						<div class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
							views
						</div>
					</div>
				</li>
			{/each}
		</ol>
	{/if}
</section>
