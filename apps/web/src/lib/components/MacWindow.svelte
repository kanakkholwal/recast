<script lang="ts">
import { cn } from "@recast/ui/utils";
import type { Snippet } from "svelte";

// macOS-style window chrome (traffic lights + titlebar), extracted from the
// hero preview and the Auto-polish editor mock so the three product mocks
// share one visual language instead of drifting. `url` renders the
// browser-style origin crumb the hero uses; omit it for plain app windows.
type Props = {
	title?: string;
	url?: string;
	class?: string;
	children: Snippet;
	// Traffic lights tint red/amber/green on hover of the window (a nice
	// macOS touch). On by default; the parent must own a `group/win`, which
	// this component provides on its root.
	hoverLights?: boolean;
};

let { title, url, class: className, children, hoverLights = true }: Props = $props();

const lights = [
	{ hover: "group-hover/win:bg-destructive/70" },
	{ hover: "group-hover/win:bg-warning/70" },
	{ hover: "group-hover/win:bg-success/70" },
];
</script>

<div
	class={cn(
		"mockup-frame group/win relative overflow-hidden",
		className,
	)}
>
	<div
		class="flex h-10 items-center gap-2 border-b border-border-low bg-paper px-4"
	>
		<div class="flex gap-1.5">
			{#each lights as light (light.hover)}
				<span
					class={cn(
						"size-2.5 rounded-full bg-border-strong",
						hoverLights && `transition-colors ${light.hover}`,
					)}
				></span>
			{/each}
		</div>
		{#if url || title}
			<div
				class="ml-3 flex items-center gap-2 text-caption font-medium text-muted-foreground"
			>
				{#if url}
					<span class="hidden sm:inline">{url}</span>
				{/if}
				{#if url && title}
					<span class="hidden sm:inline">·</span>
				{/if}
				{#if title}
					<span>{title}</span>
				{/if}
			</div>
		{/if}
	</div>
	{@render children()}
</div>
