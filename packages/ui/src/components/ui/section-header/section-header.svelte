<script lang="ts">
import type { Snippet } from "svelte";
import { cn } from "@recast/ui/utils";

type Props = {
	eyebrow?: string;
	title: string;
	description?: string;
	align?: "left" | "center";
	class?: string;
	actions?: Snippet;
};

let { eyebrow, title, description, align = "left", class: className, actions }: Props = $props();
</script>

<div
	data-slot="section-header"
	class={cn(
		"flex flex-col gap-5",
		align === "center" && "items-center text-center mx-auto max-w-2xl",
		className,
	)}
>
	{#if eyebrow}
		<!-- Sentence-case pill, not an uppercase micro-label. Uppercase +
		     letter-spaced eyebrows read as a tic once a page carries more than
		     one, and they cost legibility at 11px. -->
		<span
			class="pill inline-flex w-fit items-center gap-2 px-3 py-1 text-body-sm font-medium text-muted-foreground"
		>
			{eyebrow}
		</span>
	{/if}
	<h2 class="text-balance text-heading sm:text-heading-lg md:text-display">
		{title}
	</h2>
	{#if description}
		<p
			class={cn(
				"text-pretty text-body text-muted-foreground sm:text-body-lg",
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
