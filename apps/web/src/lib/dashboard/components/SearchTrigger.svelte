<script lang="ts">
	import { commandPalette } from "$lib/dashboard/command-palette.svelte";
	import { Kbd } from "@recast/ui/kbd";
	import { cn } from "@recast/ui/utils";
	import { Search } from "@lucide/svelte";

	// Search-bar button that opens the shared command palette. `hero` is the
	// prominent desktop-style bar (dashboard hero); `compact` fits the header.
	// Same colours either way — only the size and radius change.
	let {
		variant = "compact",
		class: className,
	}: {
		variant?: "compact" | "hero";
		class?: string;
	} = $props();

	const hero = $derived(variant === "hero");
</script>

<button
	type="button"
	onclick={() => commandPalette.show()}
	aria-label="Search pages and actions"
	title="Search (⌘K)"
	class={cn(
		"group/search flex w-full items-center text-left text-muted-foreground shadow-(--shadow-craft-inset)  hover:border-border hover:bg-card hover:shadow-craft-sm border border-border/60 bg-card/70 backdrop-blur transition-all duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/50",
		hero
			? "h-12 gap-3 rounded-xl px-4 text-[13px]"
			: "h-9 gap-2.5 rounded-lg px-3 text-sm",
		className,
	)}
>
	<Search class="size-4 shrink-0 text-muted-foreground/70 transition-colors group-hover/search:text-foreground" />
	<span class={cn("flex-1 truncate", hero && "font-medium text-muted-foreground/80")}>
		Search pages and actions…
	</span>
	<Kbd class={cn("shrink-0", !hero && "hidden sm:inline-flex")}>
		<span class="text-[9px] font-semibold">⌘</span>
		<span class="text-[10px]">K</span>
	</Kbd>
</button>
