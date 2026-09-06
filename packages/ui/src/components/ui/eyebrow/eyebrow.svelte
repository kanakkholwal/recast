<script lang="ts" module>
import { tv, type VariantProps } from "tailwind-variants";

export const eyebrowVariants = tv({
	base: "inline-flex items-center gap-2 rounded-full border px-3 py-1 text-body-sm font-medium transition-colors",
	variants: {
		variant: {
			default: "border-border bg-card text-muted-foreground",
			primary: "border-primary/25 bg-primary/8 text-primary",
			muted: "border-border bg-paper text-muted-foreground",
			outline: "border-border bg-transparent text-muted-foreground",
		},
	},
	defaultVariants: {
		variant: "default",
	},
});

export type EyebrowVariant = VariantProps<typeof eyebrowVariants>["variant"];
</script>

<script lang="ts">
	import type { Snippet } from "svelte";
	import type { HTMLAttributes } from "svelte/elements";
	import { cn, type WithElementRef } from "@recast/ui/utils";

	type Props = WithElementRef<HTMLAttributes<HTMLSpanElement>> & {
		// Permissive on purpose: accepts both functional and legacy class icon components without forcing a shape.
		// biome-ignore lint/suspicious/noExplicitAny: accepts both functional and legacy class icon components.
		icon?: any;
		variant?: EyebrowVariant;
		children: Snippet;
	};

	let {
		ref = $bindable(null),
		icon: Icon,
		variant = "default",
		class: className,
		children,
		...rest
	}: Props = $props();
</script>

<span
	bind:this={ref}
	data-slot="eyebrow"
	class={cn(eyebrowVariants({ variant }), className)}
	{...rest}
>
	{#if Icon}
		<Icon class="size-3.5" />
	{/if}
	{@render children()}
</span>
