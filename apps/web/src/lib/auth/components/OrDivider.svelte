<script lang="ts">
let { label = "or" }: { label?: string } = $props();
</script>

<div class="flex items-center gap-3">
	<span class="h-px flex-1 bg-border-low"></span>
	<span class="text-caption text-muted-foreground">{label}</span>
	<span class="h-px flex-1 bg-border-low"></span>
</div>
