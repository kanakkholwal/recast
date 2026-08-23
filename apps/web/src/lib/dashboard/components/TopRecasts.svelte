<script lang="ts">
import { BarChart3, Crown } from "@recast/icons";
import { formatCount } from "$lib/dashboard/format";
import type { Recast } from "$lib/dashboard/store.svelte";
import EmptyState from "./EmptyState.svelte";

// What is actually working, ranked. Bars are relative to the leader, so the
// shape reads at a glance without an axis.
let { recasts, limit = 4 }: { recasts: Recast[]; limit?: number } = $props();

const ranked = $derived(
	[...recasts]
		.filter((r) => r.views > 0)
		.sort((a, b) => b.views - a.views)
		.slice(0, limit),
);
const top = $derived(ranked[0]?.views ?? 0);
</script>

<section class="surface flex h-full flex-col">
	<header class="flex items-center justify-between gap-4 border-b border-border-low px-5 py-3.5">
		<div class="flex items-center gap-2">
			<Crown class="size-4 text-muted-foreground" />
			<h2 class="font-display text-body font-medium text-foreground">Top recasts</h2>
		</div>
		<a
			href="/dashboard/analytics"
			class="shrink-0 text-body-sm text-muted-foreground underline-offset-4 transition-colors hover:text-foreground hover:underline"
		>
			Analytics
		</a>
	</header>

	{#if ranked.length === 0}
		<EmptyState
			bordered={false}
			icon={BarChart3}
			title="No views yet"
			description="Share a recast and the leaders show up here."
		/>
	{:else}
		<ol class="divide-y divide-border-low">
			{#each ranked as rec, i (rec.id)}
				<li>
					<a
						href="/dashboard/recasts/{rec.id}"
						class="flex items-center gap-3 px-5 py-3 transition-colors hover:bg-paper motion-reduce:transition-none"
					>
						<span class="w-4 shrink-0 font-display font-medium text-body-sm tabular-nums text-border-strong">
							{i + 1}
						</span>
						<span class="min-w-0 flex-1">
							<span class="block truncate text-body-sm font-medium text-foreground" title={rec.title}>
								{rec.title}
							</span>
							<span
								aria-hidden="true"
								class="mt-1.5 block h-1 overflow-hidden rounded-full bg-paper"
							>
								<span
									class="block h-full rounded-full bg-foreground transition-[width] duration-700 ease-[cubic-bezier(0.625,0.05,0,1)] motion-reduce:transition-none"
									style="width: {top > 0 ? Math.round((rec.views / top) * 100) : 0}%"
								></span>
							</span>
						</span>
						<span class="shrink-0 text-body-sm tabular-nums text-muted-foreground">
							{formatCount(rec.views)}
						</span>
					</a>
				</li>
			{/each}
		</ol>
	{/if}
</section>
