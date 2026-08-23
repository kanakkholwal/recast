<script lang="ts" module>
export { flagEmoji } from "./BreakdownList.logic";
</script>

<script lang="ts">
	import type { BreakdownRow } from "$lib/dashboard/activity";
	import { formatCount } from "$lib/dashboard/format";
	import type { IconComponent } from "@recast/icons";

	let {
		title,
		icon: Icon,
		rows,
		empty = "No data yet.",
		glyph,
	}: {
		title: string;
		icon: IconComponent;
		rows: BreakdownRow[];
		empty?: string;
		/** Optional leading glyph per row (e.g. a flag emoji for geography). */
		glyph?: (row: BreakdownRow) => string;
	} = $props();
</script>

<section class="surface flex flex-col rounded-xl p-5">
	<header class="flex items-center gap-2">
		<Icon class="size-4 text-muted-foreground" />
		<h2 class="text-sm font-semibold text-foreground">{title}</h2>
	</header>

	{#if rows.length === 0}
		<p class="mt-4 text-xs text-muted-foreground">{empty}</p>
	{:else}
		<ul class="mt-4 space-y-3">
			{#each rows as r (r.key)}
				{@const g = glyph?.(r) ?? ""}
				<li>
					<div class="flex items-center justify-between gap-3 text-xs">
						<span class="flex min-w-0 items-center gap-2 text-foreground">
							{#if g}<span class="text-sm leading-none">{g}</span>{/if}
							<span class="truncate font-medium">{r.label}</span>
						</span>
						<span class="shrink-0 tabular-nums text-muted-foreground">
							{formatCount(r.count)} · {r.pct}%
						</span>
					</div>
					<div class="mt-1.5 h-1.5 overflow-hidden rounded-full bg-paper">
						<div
							class="h-full rounded-full bg-foreground transition-[width] duration-500 ease-[cubic-bezier(0.625,0.05,0,1)] motion-reduce:transition-none"
							style="width: {Math.max(2, r.pct)}%"
						></div>
					</div>
				</li>
			{/each}
		</ul>
	{/if}
</section>
