<script lang="ts" module>
	import { tv, type VariantProps } from "tailwind-variants";

	export const eyebrowVariants = tv({
		base: "inline-flex items-center gap-2 rounded-full border px-3 py-1 text-[11px] font-semibold uppercase tracking-[0.16em] shadow-(--shadow-craft-inset) transition-colors",
		variants: {
			variant: {
				default: "border-border/50 bg-card/60 text-foreground/80 backdrop-blur-md",
				primary: "border-primary/20 bg-primary/8 text-primary",
				muted: "border-border/40 bg-muted/60 text-muted-foreground",
				outline: "border-border text-foreground/80 bg-transparent",
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
		// Permissive on purpose — accepts both @lucide/svelte (Svelte 5 functional) and
		// legacy lucide-svelte (class) icon components without forcing a specific shape.
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
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
		<Icon class="size-3" />
	{/if}
	{@render children()}
</span>
