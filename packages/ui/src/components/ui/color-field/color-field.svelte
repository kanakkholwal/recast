<script lang="ts" module>
	import type { Snippet } from "svelte";

	export interface ColorFieldProps {
		/** Row label (e.g. "Color", "Background"). */
		label: string;
		/** Current color as a CSS hex string (#rrggbb or #rrggbbaa). */
		value: string;
		/** Fired when the user commits a new color from the popover. */
		oncommit: (next: string) => void;
		/** Optional preset palette shown in the popover. */
		swatches?: string[];
		/** Optional recents list shown in the popover. */
		recents?: string[];
		/** Show the alpha slider inside the popover. */
		allowAlpha?: boolean;
		/** Leading glyph rendered next to the label. */
		icon?: Snippet;
		disabled?: boolean;
		/** Bits-UI popover alignment. Defaults to `start`. */
		align?: "start" | "center" | "end";
		class?: string;
	}
</script>

<script lang="ts">
	import { ColorPicker } from "@recast/ui/color-picker";
	import * as Popover from "@recast/ui/popover";
	import { cn } from "@recast/ui/utils";

	let {
		label,
		value,
		oncommit,
		swatches,
		recents = [],
		allowAlpha = true,
		icon,
		disabled = false,
		align = "start",
		class: className,
	}: ColorFieldProps = $props();

	// Row affordance that matches <SliderControl> in geometry, typography, and
	// state — same h-10 card, same label/value rhythm, so a panel that mixes
	// sliders and color rows reads as one form. The swatch + hex on the right
	// is the "value" half; clicking anywhere on the card opens the full
	// ColorPicker in a popover so the trigger stays compact.

	// Display the hex in upper case for legibility, but keep the source value
	// untouched (consumers may store rgba()/named colors).
	const displayHex = $derived(
		value && value.startsWith("#") ? value.toUpperCase() : value,
	);
</script>

<Popover.Root>
	<Popover.Trigger>
		{#snippet child({ props })}
			<button
				type="button"
				{...props}
				{disabled}
				aria-label={`${label} — opens color picker`}
				class={cn(
					"group/field relative flex h-10 w-full select-none items-center gap-3 overflow-hidden rounded-md border border-border/40 bg-card/60 px-3 text-left outline-none transition-colors duration-150",
					"focus-visible:ring-2 focus-visible:ring-primary/30 focus-visible:ring-offset-1 focus-visible:ring-offset-background",
					disabled
						? "cursor-not-allowed opacity-50"
						: "cursor-pointer hover:border-border/60 hover:bg-card/80",
					className,
				)}
			>
				<span class="flex min-w-0 flex-1 items-center gap-1.5">
					{#if icon}
						<span class="flex size-3.5 shrink-0 items-center justify-center text-muted-foreground">
							{@render icon()}
						</span>
					{/if}
					<span class="truncate text-[12px] font-medium text-muted-foreground">
						{label}
					</span>
				</span>

				<span class="flex shrink-0 items-center gap-2">
					<!-- Pill swatch. A checker-grid sits underneath so transparent /
					     alpha-tinted colors read correctly without a confusing solid
					     background. -->
					<span
						class="relative inline-block h-4 w-7 overflow-hidden rounded-md border border-border/60 shadow-[inset_0_0_0_1px_color-mix(in_srgb,_var(--color-foreground)_4%,_transparent)]"
						aria-hidden="true"
					>
						<span
							class="absolute inset-0"
							style="background-image: conic-gradient(color-mix(in srgb, var(--color-foreground) 10%, transparent) 0deg 90deg, transparent 90deg 180deg, color-mix(in srgb, var(--color-foreground) 10%, transparent) 180deg 270deg, transparent 270deg 360deg); background-size: 6px 6px;"
						></span>
						<span class="absolute inset-0" style:background={value}></span>
					</span>
					<span class="font-mono text-[12px] font-medium tabular-nums text-foreground/85">
						{displayHex}
					</span>
				</span>
			</button>
		{/snippet}
	</Popover.Trigger>
	<Popover.Content {align} sideOffset={6} class="w-auto p-0">
		<ColorPicker {value} {swatches} {recents} {allowAlpha} {oncommit} />
	</Popover.Content>
</Popover.Root>
