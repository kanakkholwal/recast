<script lang="ts">
import type { IconComponent } from "@recast/icons";
import { cn } from "@recast/ui/utils";

export type FeatureColumn = {
	icon: IconComponent;
	title: string;
	description: string;
	href?: string;
	linkLabel?: string;
};

// The three-up detail row that closes a pillar section. One column is lit at a
// time: an accent rule on its edge and full-strength text, the others dimmed.
// Hovering or focusing a column moves the light, so the row rewards a scan
// instead of presenting three equally-loud blocks.
let {
	items,
	accent = "primary",
	initial = 0,
	class: className = "",
}: {
	items: FeatureColumn[];
	accent?: "tangerine" | "lavender" | "green" | "primary";
	initial?: number;
	class?: string;
} = $props();

let active = $state(initial);

const rule = {
	tangerine: "bg-tag-tangerine",
	lavender: "bg-tag-lavender",
	green: "bg-tag-green",
	primary: "bg-primary",
} as const;

const link = {
	tangerine: "text-tag-tangerine",
	lavender: "text-tag-lavender",
	green: "text-tag-green",
	primary: "text-primary",
} as const;
</script>

<div class={cn("grid border-t border-border-low sm:grid-cols-3", className)}>
	{#each items as item, i (item.title)}
		{@const on = active === i}
		{@const Icon = item.icon}
		<div
			role="group"
			class="relative border-border-low px-6 py-8 sm:border-l sm:first:border-l-0 sm:px-8"
			onmouseenter={() => (active = i)}
			onfocusin={() => (active = i)}
		>
			<!-- Accent rule. Absolute so lighting a column never shifts the row. -->
			<span
				aria-hidden="true"
				class={cn(
					"absolute inset-y-0 left-0 hidden w-0.5 transition-opacity duration-300 motion-reduce:transition-none sm:block",
					rule[accent],
				)}
				style={`opacity:${on ? 1 : 0}`}
			></span>

			<Icon
				class={cn(
					"size-4 transition-colors duration-300",
					on ? "text-foreground" : "text-muted-foreground",
				)}
			/>
			<h3
				class={cn(
					"mt-4 text-body font-semibold transition-colors duration-300",
					on ? "text-foreground" : "text-muted-foreground",
				)}
			>
				{item.title}
			</h3>
			<p class="mt-2 max-w-xs text-body-sm text-muted-foreground">
				{item.description}
			</p>
			{#if item.href}
				<a
					href={item.href}
					class={cn(
						"mt-4 inline-flex items-center gap-1 text-body-sm font-medium transition-colors duration-300",
						on ? link[accent] : "text-muted-foreground",
					)}
				>
					{item.linkLabel ?? "Learn more"}
					<span aria-hidden="true">›</span>
				</a>
			{/if}
		</div>
	{/each}
</div>
