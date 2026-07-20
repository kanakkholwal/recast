<script lang="ts">
	import type { IconComponent } from "@recast/icons";

	let {
		icon: Icon,
		label,
		value,
	}: {
		icon: IconComponent;
		label: string;
		value: string;
	} = $props();
</script>

<div class="glass-card flex items-center gap-3.5 rounded-xl p-4">
	<span class="glass-chip grid size-10 place-items-center rounded-lg text-muted-foreground">
		<Icon class="size-4" />
	</span>
	<div>
		<div class="text-lg font-semibold tabular-nums tracking-tight text-foreground">
			{value}
		</div>
		<div class="text-[11px] font-medium uppercase tracking-[0.14em] text-muted-foreground">
			{label}
		</div>
	</div>
</div>
