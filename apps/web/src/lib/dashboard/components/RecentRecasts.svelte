<script lang="ts">
import { formatDuration, formatRelative } from "$lib/dashboard/format";
import type { Recast } from "$lib/dashboard/store.svelte";
import { Clock, Film, Play } from "@recast/icons";
import EmptyState from "./EmptyState.svelte";

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

<section class="surface flex h-full flex-col">
	<header class="flex items-center justify-between gap-4 border-b border-border-low px-5 py-3.5">
		<div class="flex items-center gap-2">
			<Film class="size-4 text-muted-foreground" />
			<h2 class="font-display text-body font-medium text-foreground">Recent recasts</h2>
		</div>
		<a
			href="/dashboard/recasts"
			class="shrink-0 text-body-sm text-muted-foreground underline-offset-4 transition-colors hover:text-foreground hover:underline"
		>
			View all
		</a>
	</header>

	{#if items.length === 0}
		<EmptyState bordered={false} icon={Film} title="No recasts yet" description="Upload one to see it here." />
	{:else}
		<div class="grid grid-cols-2 gap-3 p-4 sm:grid-cols-4">
			{#each items as rec (rec.id)}
				<a href="/dashboard/recasts/{rec.id}" class="group/tile flex flex-col gap-2 text-left">
					<div class="relative aspect-video overflow-hidden rounded-lg border border-border-low bg-paper">
						{#if rec.posterUrl && !failed[rec.id]}
							<img
								src={rec.posterUrl}
								alt=""
								loading="lazy"
								onerror={() => (failed = { ...failed, [rec.id]: true })}
								class="absolute inset-0 size-full object-cover"
							/>
						{:else}
							<div class="absolute inset-0 grid place-items-center">
								<Film class="size-6 text-border-strong" />
							</div>
						{/if}
						<span class="absolute inset-0 grid place-items-center bg-background/40 opacity-0 transition-opacity duration-300 group-hover/tile:opacity-100 motion-reduce:transition-none">
							<span class="grid size-9 place-items-center rounded-full bg-foreground text-background shadow-craft-floating">
								<Play class="size-4 translate-x-0.5 fill-current" />
							</span>
						</span>
						<span class="absolute bottom-1.5 right-1.5 flex items-center gap-1 rounded border border-border-low bg-background px-1.5 py-0.5 text-caption tabular-nums text-foreground">
							<Clock class="size-2.5" />{formatDuration(rec.durationSec)}
						</span>
					</div>
					<div class="min-w-0">
						<p class="truncate text-body-sm font-medium text-foreground" title={rec.title}>{rec.title}</p>
						<p class="truncate text-caption text-muted-foreground">{formatRelative(rec.createdAt)}</p>
					</div>
				</a>
			{/each}
		</div>
	{/if}
</section>
