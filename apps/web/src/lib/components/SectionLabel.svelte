<script lang="ts">
import type { IconComponent } from "@recast/icons";
import { cn } from "@recast/ui/utils";

let {
	icon: Icon,
	label,
	accent = "neutral",
	class: className = "",
}: {
	icon?: IconComponent;
	label: string;
	accent?: "tangerine" | "lavender" | "green" | "primary" | "neutral";
	class?: string;
} = $props();

const accentClass = {
	tangerine: "text-tag-tangerine",
	lavender: "text-tag-lavender",
	green: "text-tag-green",
	primary: "text-primary",
	neutral: "text-muted-foreground",
} as const;
</script>

<span class={cn("inline-flex items-center gap-2 text-body-sm font-medium text-foreground", className)}>
	{#if Icon}
		<Icon class={cn("size-4 shrink-0 [fill-opacity:0.2]", accentClass[accent])} fill="currentColor" />
	{/if}
	{label}
</span>
