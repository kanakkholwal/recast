<script lang="ts">
// Shared by the workspace analytics page and the per-recast page so the control behaves identically.
let {
	value = $bindable("7d"),
	options,
}: {
	value?: string;
	options: { label: string; value: string }[];
} = $props();
</script>

<div class="flex items-center gap-0.5 rounded-lg border border-border-low bg-paper p-0.5" role="radiogroup">
	{#each options as o (o.value)}
		<button
			type="button"
			role="radio"
			onclick={() => (value = o.value)}
			aria-checked={value === o.value}
			class="rounded-md px-3 py-1.5 text-body-sm font-medium transition-colors duration-200 motion-reduce:transition-none
				{value === o.value
					? 'bg-background text-foreground shadow-craft-sm'
					: 'text-muted-foreground hover:text-foreground'}"
		>
			{o.label}
		</button>
	{/each}
</div>
