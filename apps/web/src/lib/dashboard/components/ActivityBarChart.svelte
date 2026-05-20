<script lang="ts">
	let {
		data,
	}: {
		data: { label: string; views: number }[];
	} = $props();

	const max = $derived(Math.max(1, ...data.map((d) => d.views)));
	const total = $derived(data.reduce((s, d) => s + d.views, 0));
</script>

<div class="rounded-xl border border-border-low/50 bg-background/40 p-5 backdrop-blur-sm">
	<div class="flex items-end justify-between">
		<div>
			<div class="font-mono text-2xl font-semibold tabular-nums tracking-tight text-foreground">
				{total}
			</div>
			<div class="text-[10px] font-semibold uppercase tracking-[0.14em] text-muted-foreground">
				Views in range
			</div>
		</div>
		<div class="text-right text-[10px] font-semibold uppercase tracking-[0.14em] text-muted-foreground">
			Peak · {max}
		</div>
	</div>

	<div class="mt-5 flex h-32 items-end gap-1.5">
		{#each data as d, i (i)}
			{@const h = Math.max(2, Math.round((d.views / max) * 100))}
			<div class="group/bar relative flex h-full flex-1 flex-col items-center justify-end">
				<div
					class="w-full origin-bottom rounded-t-sm bg-linear-to-t from-primary/30 to-primary/80 ring-1 ring-inset ring-primary/10 transition-all duration-300 ease-[cubic-bezier(0.625,0.05,0,1)] group-hover/bar:from-primary/45 group-hover/bar:to-primary"
					style="height: {h}%"
				></div>
				<div
					class="pointer-events-none absolute -top-7 left-1/2 -translate-x-1/2 rounded-md bg-foreground px-1.5 py-0.5 font-mono text-[10px] font-semibold text-background opacity-0 transition-opacity duration-200 group-hover/bar:opacity-100"
				>
					{d.views}
				</div>
			</div>
		{/each}
	</div>

	<div class="mt-3 flex justify-between text-[10px] font-medium text-muted-foreground">
		{#each data as d, i (i)}
			<span class="flex-1 text-center">{d.label}</span>
		{/each}
	</div>
</div>
