<script lang="ts">
	import { formatDuration, formatRelative } from "$lib/dashboard/format";
	import type { Recast } from "$lib/dashboard/store.svelte";
	import EmptyState from "./EmptyState.svelte";
	import { Clock, Film, Play } from "@lucide/svelte";

	// Visual recent-recasts rail for the home overview — poster thumbnails that
	// link through to each recast. A warmer, more product-forward counterpart to
	// the text-only "Top recasts" list.
	let {
		recasts,
		limit = 4,
	}: {
		recasts: Recast[];
		limit?: number;
	} = $props();

	const items = $derived(recasts.slice(0, limit));
	let failed = $state<Record<string, boolean>>({});
</script>

<section class="glass-card flex h-full flex-col rounded-xl">
	<header class="flex items-center justify-between border-b border-border-low/50 px-5 py-3.5">
		<div class="flex items-center gap-2">
			<Film class="size-4 text-primary" />
			<h2 class="text-sm font-semibold text-foreground">Recent recasts</h2>
		</div>
		<a
			href="/dashboard/recasts"
			class="text-[11px] font-semibold uppercase tracking-[0.14em] text-muted-foreground transition-colors hover:text-foreground"
		>
			View all →
		</a>
	</header>

	{#if items.length === 0}
		<EmptyState bordered={false} icon={Film} title="No recasts yet" description="Upload one to see it here." />
	{:else}
		<div class="grid grid-cols-2 gap-3 p-4 sm:grid-cols-4">
			{#each items as rec (rec.id)}
				<a href="/dashboard/recasts/{rec.id}" class="group/tile flex flex-col gap-2 text-left">
					<div class="relative aspect-video overflow-hidden rounded-lg bg-foreground/5 ring-1 ring-inset ring-border-low/40">
						{#if rec.posterUrl && !failed[rec.id]}
							<img
								src={rec.posterUrl}
								alt=""
								loading="lazy"
								onerror={() => (failed = { ...failed, [rec.id]: true })}
								class="absolute inset-0 size-full object-cover transition-transform duration-500 group-hover/tile:scale-105"
							/>
						{:else}
							<div class="absolute inset-0 grid place-items-center">
								<Film class="size-6 text-foreground/30" />
							</div>
						{/if}
						<span class="absolute inset-0 grid place-items-center bg-background/30 opacity-0 backdrop-blur-[1px] transition-opacity duration-300 group-hover/tile:opacity-100">
							<span class="grid size-9 place-items-center rounded-full bg-primary text-background shadow-craft-floating">
								<Play class="size-4 translate-x-0.5 fill-current" />
							</span>
						</span>
						<span class="absolute bottom-1.5 right-1.5 flex items-center gap-0.5 rounded bg-background/85 px-1 py-0.5 font-mono text-[9px] font-semibold tabular-nums text-foreground ring-1 ring-inset ring-border-low/50 backdrop-blur-sm">
							<Clock class="size-2.5" />{formatDuration(rec.durationSec)}
						</span>
					</div>
					<div class="min-w-0">
						<p class="truncate text-xs font-medium text-foreground" title={rec.title}>{rec.title}</p>
						<p class="truncate text-[10px] text-muted-foreground">{formatRelative(rec.createdAt)}</p>
					</div>
				</a>
			{/each}
		</div>
	{/if}
</section>
