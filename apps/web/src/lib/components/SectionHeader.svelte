<script lang="ts">
	import { cn } from "@recast/ui/utils";
	import type { Snippet } from "svelte";

	let {
		eyebrow,
		title,
		description,
		align = "left",
		class: className = "",
		actions,
	}: {
		eyebrow?: string;
		title: string;
		description?: string;
		align?: "left" | "center";
		class?: string;
		actions?: Snippet;
	} = $props();
</script>

<div
	class={cn(
		"flex flex-col gap-5",
		align === "center" && "items-center text-center mx-auto max-w-2xl",
		className,
	)}
>
	{#if eyebrow}
		<span
			class="inline-flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.18em] text-muted-foreground"
		>
			<span class="size-1.5 rounded-full bg-primary"></span>
			{eyebrow}
		</span>
	{/if}
	<h2
		class="text-balance text-3xl font-semibold tracking-tight text-foreground sm:text-4xl md:text-5xl"
	>
		{title}
	</h2>
	{#if description}
		<p
			class={cn(
				"text-pretty text-base text-muted-foreground sm:text-lg",
				align === "center" ? "max-w-xl" : "max-w-2xl",
			)}
		>
			{description}
		</p>
	{/if}
	{#if actions}
		<div class={cn("mt-2 flex flex-wrap gap-3", align === "center" && "justify-center")}>
			{@render actions()}
		</div>
	{/if}
</div>
