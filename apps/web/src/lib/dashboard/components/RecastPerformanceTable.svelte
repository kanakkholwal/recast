<script lang="ts">
import { ArrowDown, ArrowUp, BarChart3, Crown, Film } from "@recast/icons";
import EmptyState from "./EmptyState.svelte";
import {
	PERF_COLUMNS as cols,
	nextSort,
	type Row,
	type SortKey,
	sortRows,
} from "./RecastPerformanceTable.logic";

// Sortable per-recast comparison; each row drills into the recast's own page.
let { rows, limit = 25 }: { rows: Row[]; limit?: number } = $props();

let sortKey = $state<SortKey>("views");
let dir = $state<"asc" | "desc">("desc");
// Per-row poster load failures, so a broken cover falls back to the glyph.
let failed = $state<Record<string, boolean>>({});

const sorted = $derived(sortRows(rows, sortKey, dir, limit));
const hidden = $derived(Math.max(0, rows.length - sorted.length));

function toggleSort(k: SortKey) {
	({ key: sortKey, dir } = nextSort({ key: sortKey, dir }, k));
}
</script>

<section class="surface flex h-full flex-col">
	<header class="flex items-center gap-2 border-b border-border-low px-5 py-3.5">
		<Crown class="size-4 text-muted-foreground" />
		<h2 class="font-display text-body font-medium text-foreground">Every recast</h2>
	</header>

	{#if rows.length === 0}
		<EmptyState
			bordered={false}
			icon={BarChart3}
			title="No performance data yet"
			description="Share a recast to start gathering views."
		/>
	{:else}
		<div class="overflow-x-auto">
			<table class="w-full text-body-sm">
				<caption class="sr-only">
					Recasts ranked by {cols.find((c) => c.key === sortKey)?.label ?? "views"},
					{dir === "desc" ? "highest first" : "lowest first"}.
				</caption>
				<thead>
					<tr class="border-b border-border-low bg-paper text-caption text-muted-foreground">
						<th scope="col" class="px-5 py-2 text-left font-medium">Recast</th>
						{#each cols as c (c.key)}
							{@const active = sortKey === c.key}
							<th
								scope="col"
								class="px-3 py-2 text-right font-medium last:pr-5"
								aria-sort={active ? (dir === "asc" ? "ascending" : "descending") : "none"}
							>
								<button
									type="button"
									onclick={() => toggleSort(c.key)}
									class="ml-auto inline-flex min-h-8 items-center gap-1 transition-colors hover:text-foreground motion-reduce:transition-none {active
										? 'text-foreground'
										: ''}"
								>
									{c.label}
									{#if active}
										{#if dir === "desc"}
											<ArrowDown class="size-3" aria-hidden="true" />
										{:else}
											<ArrowUp class="size-3" aria-hidden="true" />
										{/if}
									{/if}
								</button>
							</th>
						{/each}
					</tr>
				</thead>
				<tbody>
					{#each sorted as r (r.id)}
						<tr
							class="border-b border-border-low transition-colors last:border-0 hover:bg-paper motion-reduce:transition-none"
						>
							<td class="max-w-0 px-5 py-2.5">
								<a
									href={`/dashboard/recasts/${r.id}`}
									class="group/row flex min-w-0 items-center gap-3"
									title={r.title}
								>
									<span
										class="relative h-9 w-16 shrink-0 overflow-hidden rounded-md border border-border-low bg-paper"
									>
										{#if r.posterUrl && !failed[r.id]}
											<img
												src={r.posterUrl}
												alt=""
												loading="lazy"
												onerror={() => (failed = { ...failed, [r.id]: true })}
												class="h-full w-full object-cover"
											/>
										{:else}
											<span class="grid h-full w-full place-items-center">
												<Film class="size-3.5 text-border-strong" />
											</span>
										{/if}
									</span>
									<span
										class="min-w-0 truncate font-medium text-foreground underline-offset-4 group-hover/row:underline"
									>
										{r.title}
									</span>
								</a>
							</td>
							{#each cols as c (c.key)}
								<td class="px-3 py-2.5 text-right tabular-nums text-muted-foreground last:pr-5">
									{c.fmt(r)}
								</td>
							{/each}
						</tr>
					{/each}
				</tbody>
			</table>
		</div>

		{#if hidden > 0}
			<!-- Say what was dropped: a silently capped table reads as the whole list. -->
			<p class="border-t border-border-low px-5 py-2.5 text-caption text-muted-foreground">
				Showing the top {sorted.length} of {rows.length} recasts.
			</p>
		{/if}
	{/if}
</section>
