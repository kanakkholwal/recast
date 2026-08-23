<script lang="ts">
import type { IconComponent } from "@recast/icons";
import { cn } from "@recast/ui/utils";
import type { Snippet } from "svelte";

let {
	icon: Icon,
	title,
	description,
	accent = false,
	badge,
	children,
}: {
	icon: IconComponent;
	title: string;
	description?: string;
	/** Emphasises the card. Marks the plan the workspace is on, or is being sold. */
	accent?: boolean;
	badge?: Snippet;
	children: Snippet;
} = $props();
</script>

<section class={cn("surface p-6", accent && "border-border-strong")}>
	<div class="flex items-start gap-3">
		<Icon class="mt-0.5 size-5 shrink-0 text-muted-foreground" />
		<div class="min-w-0 flex-1">
			<!-- Badge rides the title row, or it crushes the description in a narrow rail. -->
			<div class="flex items-center justify-between gap-3">
				<h2 class="min-w-0 font-display text-body font-medium text-foreground">{title}</h2>
				{#if badge}
					{@render badge()}
				{/if}
			</div>
			{#if description}
				<p class="mt-0.5 text-pretty text-body-sm text-muted-foreground">{description}</p>
			{/if}
		</div>
	</div>
	<div class="mt-5">
		{@render children()}
	</div>
</section>
