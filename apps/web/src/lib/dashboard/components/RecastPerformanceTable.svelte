<script lang="ts">
	import EmptyState from "./EmptyState.svelte";
	import {
		nextSort,
		PERF_COLUMNS as cols,
		sortRows,
		type Row,
		type SortKey,
	} from "./RecastPerformanceTable.logic";
	import { ArrowDown, ArrowUp, BarChart3, Crown } from "@lucide/svelte";

	// Sortable per-recast comparison table. Replaces the text-only "Top recasts"
	// on the analytics page; each row drills into /dashboard/recasts/[id].
	let { rows, limit = 25 }: { rows: Row[]; limit?: number } = $props();

	let sortKey = $state<SortKey>("views");
	let dir = $state<"asc" | "desc">("desc");

	const sorted = $derived(sortRows(rows, sortKey, dir, limit));

	function toggleSort(k: SortKey) {
		({ key: sortKey, dir } = nextSort({ key: sortKey, dir }, k));
	}
</script>

<section class="glass-card flex h-full flex-col rounded-xl">
	<header class="flex items-center gap-2 border-b border-border-low/50 px-5 py-3.5">
		<Crown class="size-4 text-primary" />
		<h2 class="text-sm font-semibold text-foreground">Recast performance</h2>
	</header>

	{#if rows.length === 0}
		<EmptyState bordered={false} icon={BarChart3} title="No performance data yet" description="Share a recast to start gathering views." />
	{:else}
		<div class="overflow-x-auto">
			<table class="w-full text-sm">
				<thead>
					<tr class="border-b border-border-low/40 text-[10px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
						<th class="px-5 py-2 text-left font-semibold">Recast</th>
						{#each cols as c (c.key)}
							<th class="px-3 py-2 text-right font-semibold last:pr-5">
								<button
									type="button"
									onclick={() => toggleSort(c.key)}
									class="ml-auto inline-flex items-center gap-1 transition-colors hover:text-foreground {sortKey === c.key ? 'text-foreground' : ''}"
								>
									{c.label}
									{#if sortKey === c.key}
										{#if dir === "desc"}<ArrowDown class="size-3" />{:else}<ArrowUp class="size-3" />{/if}
									{/if}
								</button>
							</th>
						{/each}
					</tr>
				</thead>
				<tbody>
					{#each sorted as r (r.id)}
						<tr class="border-b border-border-low/20 transition-colors last:border-0 hover:bg-foreground/3">
							<td class="max-w-0 px-5 py-2.5">
								<a href={`/dashboard/recasts/${r.id}`} class="block truncate font-medium text-foreground hover:text-primary" title={r.title}>
									{r.title}
								</a>
							</td>
							{#each cols as c (c.key)}
								<td class="px-3 py-2.5 text-right font-mono tabular-nums text-muted-foreground last:pr-5">
									{c.fmt(r)}
								</td>
							{/each}
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
</section>
