<script lang="ts" module>
	import { cn, type WithElementRef } from "@recast/utils";
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
			"group/button inline-flex shrink-0 items-center justify-center gap-2 whitespace-nowrap cursor-pointer",
			"rounded-md border border-transparent bg-clip-padding font-medium outline-none transition-all select-none",
			"focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-3",
			"aria-invalid:border-destructive aria-invalid:ring-destructive/20 aria-invalid:ring-3",
			"dark:aria-invalid:border-destructive/50 dark:aria-invalid:ring-destructive/40",
			"active:not-aria-[haspopup]:translate-y-px",
			"disabled:pointer-events-none disabled:opacity-50",
			"[&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
		].join(" "),
		variants: {
			variant: {
				default: "bg-primary text-primary-foreground shadow-xs hover:bg-primary/90",
				default_soft:
					"bg-primary/10 text-primary hover:bg-primary/15 dark:bg-primary/10 dark:hover:bg-primary/20",
				secondary:
					"bg-secondary text-secondary-foreground shadow-xs hover:bg-secondary/80",
				outline:
					"border-border bg-background/40 shadow-xs hover:bg-accent hover:text-accent-foreground dark:bg-input/30 dark:hover:bg-input/50",
				ghost:
					"hover:bg-accent hover:text-accent-foreground dark:hover:bg-accent/50",
				link: "text-primary underline-offset-4 hover:underline",
				destructive:
					"bg-destructive text-destructive-foreground shadow-xs hover:bg-destructive/90 focus-visible:ring-destructive/20 dark:bg-destructive/60 dark:focus-visible:ring-destructive/40",
				destructive_soft:
					"bg-destructive/10 text-destructive hover:bg-destructive/15 dark:bg-destructive/10 dark:hover:bg-destructive/20",
				success: "bg-success text-success-foreground shadow-xs hover:bg-success/90",
				success_soft:
					"bg-success/10 text-success hover:bg-success/15 dark:bg-success/10 dark:hover:bg-success/20",
				warning: "bg-warning text-warning-foreground shadow-xs hover:bg-warning/90",
				warning_soft:
					"bg-warning/10 text-warning hover:bg-warning/15 dark:bg-warning/10 dark:hover:bg-warning/20",
				info: "bg-info text-info-foreground shadow-xs hover:bg-info/90",
				info_soft:
					"bg-info/10 text-info hover:bg-info/15 dark:bg-info/10 dark:hover:bg-info/20",
				// Chromeless button — inherits typography, no padding. For custom layouts that still want focus/active behavior.
				raw: "",
			},
			size: {
				default: "h-9 rounded-lg px-5 py-2.5 text-sm has-[>svg]:px-3",
				lg: "h-10 rounded-lg px-6 text-sm has-[>svg]:px-4",
				sm: "h-8 rounded-md px-3 text-xs has-[>svg]:px-2.5",
				xs: "h-6 rounded-md px-2 text-[11px] gap-1.5 has-[>svg]:px-2 [&_svg:not([class*='size-'])]:size-3",
				icon: "size-9 rounded-lg",
				"icon-sm": "size-8 rounded-md",
				"icon-xs": "size-6 rounded-md [&_svg:not([class*='size-'])]:size-3",
				"icon-lg": "size-10 rounded-lg",
				// Chromeless size — no height/padding. Use with className when the button must match an arbitrary parent (e.g. window chrome).
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
