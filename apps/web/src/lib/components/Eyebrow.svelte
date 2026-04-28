<script lang="ts">
	import { cn } from "@recast/ui/utils";
	import type { Icon as LucideIcon } from "lucide-svelte";
	import type { Snippet } from "svelte";

	let {
		children,
		icon: Icon,
		class: className = "",
		variant = "default",
	}: {
		children: Snippet;
		icon?: typeof LucideIcon;
		class?: string;
		variant?: "default" | "primary" | "muted";
	} = $props();

	const variants = {
		default:
			"border-border-low bg-card/60 text-foreground/80 backdrop-blur-md",
		primary: "border-primary/20 bg-primary/8 text-primary",
		muted: "border-border-low bg-muted/60 text-muted-foreground",
	};
</script>

<span
	class={cn(
		"inline-flex items-center gap-2 rounded-full border px-3 py-1 text-[11px] font-semibold uppercase tracking-[0.16em] shadow-craft-sm",
		variants[variant],
		className,
	)}
>
	{#if Icon}
		<Icon class="size-3" />
	{/if}
	{@render children()}
</span>
