<script lang="ts">
import { Search } from "@recast/icons";
import { Kbd } from "@recast/ui/kbd";
import { cn } from "@recast/ui/utils";
import { commandPalette } from "$lib/dashboard/command-palette.svelte";

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
	"group/search flex items-center border border-border-low bg-paper text-left text-muted-foreground transition-colors duration-200 hover:border-border-strong hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50 motion-reduce:transition-none",
		hero
			? "h-12 w-full gap-3 rounded-xl px-4 text-body"
			: "h-9 w-9 shrink-0 justify-center gap-2.5 rounded-lg text-body-sm sm:w-full sm:justify-start sm:px-3",
		className,
	)}
>
	<Search class="size-4 shrink-0 transition-colors group-hover/search:text-foreground" />
	<span class={cn("flex-1 truncate text-muted-foreground", hero ? "" : "hidden sm:block")}>
		Search pages and actions…
	</span>
	<Kbd class={cn("shrink-0", !hero && "hidden sm:inline-flex")}>
		<span class="text-caption">⌘</span>
		<span class="text-caption">K</span>
	</Kbd>
</button>
