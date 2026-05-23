<script lang="ts" module>
	import { cn, type WithElementRef } from "@recast/ui/utils";
	import type { HTMLAnchorAttributes, HTMLButtonAttributes } from "svelte/elements";
	import { tv, type VariantProps } from "tailwind-variants";

	/**
	 * Button — Raycast-density variants built on shadcn semantic tokens.
	 *
	 * Guidelines:
	 * - Use `size="xs"` for dense toolbar rows (11px text, 24px height).
	 * - Use `size="sm"` for secondary actions (12px text, 32px height).
	 * - Use `size="default"` only for primary CTAs in forms / empty states.
	 * - Colors must come from semantic tokens — never hardcoded (emerald, sky…).
	 * - Prefer `ghost` + `icon-sm` for toolbar icon buttons.
	 */
	export const buttonVariants = tv({
		base: [
			"group/button inline-flex shrink-0 items-center justify-center gap-2 whitespace-nowrap cursor-pointer user-select-none",
			"rounded-md border border-border/40 bg-clip-padding font-medium outline-none transition-all duration-200 select-none",
			"focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-3",
			"aria-invalid:border-destructive aria-invalid:ring-destructive/20 aria-invalid:ring-3",
			"dark:aria-invalid:border-destructive/50 dark:aria-invalid:ring-destructive/40",
			"active:scale-[0.98] hover:scale-[1.02]",
			"disabled:pointer-events-none disabled:opacity-50",
			"[&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
		].join(" "),
		variants: {
			variant: {
				default: "bg-primary text-primary-foreground border-transparent shadow-craft-sm hover:bg-primary/95",
				default_soft:
					"bg-primary/8 text-primary border-primary/10 hover:bg-primary/12 dark:bg-primary/10 dark:hover:bg-primary/20",
				secondary:
					"border-secondary bg-secondary text-secondary-foreground border-border/30 shadow-craft-sm hover:bg-muted/50",
				outline:
					"border-accent bg-accent shadow-craft-sm hover:bg-accent/10 hover:text-accent-foreground dark:bg-input/30 dark:hover:bg-input/50",
				ghost:
					"border-transparent hover:bg-muted/40 hover:text-accent-foreground dark:hover:bg-accent/50",
				link: "text-primary underline-offset-4 hover:underline hover:scale-100",
				destructive:
					"bg-destructive text-destructive-foreground shadow-craft-sm hover:bg-destructive/90 focus-visible:ring-destructive/20",
				destructive_soft:
					"bg-destructive/10 text-destructive border-destructive/10 hover:bg-destructive/15",
				success: "bg-success text-success-foreground shadow-craft-sm hover:bg-success/90",
				success_soft:
					"bg-success/10 text-success border-success/10 hover:bg-success/15",
				warning: "bg-warning text-warning-foreground shadow-craft-sm hover:bg-warning/90",
				warning_soft:
					"bg-warning/10 text-warning border-warning/10 hover:bg-warning/15",
				info: "bg-info text-info-foreground shadow-craft-sm hover:bg-info/90",
				info_soft:
					"bg-info/10 text-info border-info/10 hover:bg-info/15",
				raw: "border-0 p-0 h-auto w-auto hover:scale-100 active:scale-100",
			},
			size: {
				default: "h-9 rounded-xl px-5 py-2.5 text-sm",
				lg: "h-11 rounded-2xl px-8 text-base font-semibold",
				sm: "h-8 rounded-lg px-3 text-xs",
				xs: "h-6 rounded-md px-2 text-[11px] gap-1.5 [&_svg:not([class*='size-'])]:size-3",
				icon: "size-9 rounded-xl",
				"icon-sm": "size-8 rounded-lg",
				"icon-xs": "size-6 rounded-md [&_svg:not([class*='size-'])]:size-3",
				"icon-lg": "size-11 rounded-2xl",
				raw: "",
			},
		},
		defaultVariants: {
			variant: "default",
			size: "default",
		},
	});

	export type ButtonVariant = VariantProps<typeof buttonVariants>["variant"];
	export type ButtonSize = VariantProps<typeof buttonVariants>["size"];

	export type ButtonProps = WithElementRef<HTMLButtonAttributes> &
		WithElementRef<HTMLAnchorAttributes> & {
			variant?: ButtonVariant;
			size?: ButtonSize;
		};
</script>

<script lang="ts">
	let {
		class: className,
		variant = "default",
		size = "default",
		ref = $bindable(null),
		href = undefined,
		type = "button",
		disabled,
		children,
		...restProps
	}: ButtonProps = $props();
</script>

{#if href}
	<a
		bind:this={ref}
		data-slot="button"
		class={cn(buttonVariants({ variant, size }), className)}
		href={disabled ? undefined : href}
		aria-disabled={disabled}
		role={disabled ? "link" : undefined}
		tabindex={disabled ? -1 : undefined}
		{...restProps}
	>
		{@render children?.()}
	</a>
{:else}
	<button
		bind:this={ref}
		data-slot="button"
		class={cn(buttonVariants({ variant, size }), className)}
		{type}
		{disabled}
		{...restProps}
	>
		{@render children?.()}
	</button>
{/if}
