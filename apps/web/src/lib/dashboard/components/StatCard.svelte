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

<div class="surface flex items-center gap-3.5 p-4">
	<Icon class="size-5 shrink-0 text-muted-foreground" />
	<div class="min-w-0">
		<div class="text-subheading font-medium tabular-nums text-foreground">
			{value}
		</div>
		<div class="truncate text-caption text-muted-foreground">{label}</div>
	</div>
</div>
