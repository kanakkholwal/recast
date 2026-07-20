<script lang="ts">
	// Small segmented range selector, shared by the workspace analytics page and
	// the per-recast detail page so the control looks and behaves identically.
	let {
		value = $bindable("7d"),
		options,
	}: {
		value?: string;
		options: { label: string; value: string }[];
	} = $props();
</script>

<div class="flex items-center gap-1 rounded-lg border border-border-low/60 bg-card/40 p-1">
	{#each options as o (o.value)}
		<button
			type="button"
			onclick={() => (value = o.value)}
			aria-pressed={value === o.value}
			class="rounded-md px-3 py-1.5 text-xs font-semibold transition-colors duration-200
				{value === o.value
					? 'bg-background text-foreground shadow-craft-sm'
					: 'text-muted-foreground hover:text-foreground'}"
		>
			{o.label}
		</button>
	{/each}
</div>
