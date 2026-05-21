<script lang="ts">
	let { label = "or" }: { label?: string } = $props();
</script>

<div class="flex items-center gap-3">
	<span class="h-px flex-1 bg-border-low/60"></span>
	<span class="text-[10px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
		{label}
	</span>
	<span class="h-px flex-1 bg-border-low/60"></span>
</div>
