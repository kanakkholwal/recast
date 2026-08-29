<script lang="ts" module>
import type { Snippet } from "svelte";

export interface ColorFieldProps {
	/** Row label (e.g. "Color", "Background"). */
	label: string;
	/** Current color as a CSS hex string (#rrggbb or #rrggbbaa). */
	value: string;
	/** Fired when the user commits a new color from the popover or hex edit. */
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
	/** Compact h-8 variant on the shared field surface, for inline panel rows. */
	dense?: boolean;
	/** Drop the in-field label (swatch leads) when an outer row labels it. */
	hideLabel?: boolean;
	class?: string;
}

const HEX_REGEX = /^#([0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/;
</script>

<script lang="ts">
	import { tick } from "svelte";
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
		dense = false,
		hideLabel = false,
		class: className,
	}: ColorFieldProps = $props();

	// Row affordance matches <SliderControl> in geometry, typography, and
	// state. The hex text is its own affordance — hovering it for 800ms
	// reveals an editable pill (DialKit's hover-to-type pattern). Clicking
	// the swatch (or any non-hex region) opens the full picker popover.

	const HOVER_REVEAL_MS = 800;

	let isHexHovered = $state(false);
	let isHexEditable = $state(false);
	let showInput = $state(false);
	let inputValue = $state("");
	let inputEl: HTMLInputElement | null = $state(null);
	let hoverTimer: ReturnType<typeof setTimeout> | null = null;

	const displayHex = $derived(
		value && value.startsWith("#") ? value.toUpperCase() : value,
	);

	$effect(() => {
		if (hoverTimer) {
			clearTimeout(hoverTimer);
			hoverTimer = null;
		}
		if (isHexHovered && !showInput && !isHexEditable && !disabled) {
			hoverTimer = setTimeout(() => {
				isHexEditable = true;
			}, HOVER_REVEAL_MS);
		} else if (!isHexHovered && !showInput) {
			isHexEditable = false;
		}
		return () => {
			if (hoverTimer) {
				clearTimeout(hoverTimer);
				hoverTimer = null;
			}
		};
	});

	$effect(() => {
		if (showInput) {
			tick().then(() => {
				inputEl?.focus();
				inputEl?.select();
			});
		}
	});

	function startHexEdit() {
		showInput = true;
		inputValue = displayHex;
	}

	function commitHexInput() {
		const v = inputValue.trim();
		const withHash = v.startsWith("#") ? v : `#${v}`;
		if (HEX_REGEX.test(withHash)) {
			oncommit(withHash);
		}
		showInput = false;
		isHexHovered = false;
		isHexEditable = false;
	}

	function handleHexKeydown(e: KeyboardEvent) {
		if (e.key === "Enter") {
			e.preventDefault();
			commitHexInput();
		} else if (e.key === "Escape") {
			e.preventDefault();
			showInput = false;
			isHexHovered = false;
			isHexEditable = false;
		}
	}
</script>

<Popover.Root>
	<div
		class={cn(
			"group/field relative flex w-full select-none items-center overflow-hidden text-left outline-none",
			dense
				? "h-8 gap-2 rounded-lg bg-muted/60 px-2 ring-1 ring-inset ring-border/40 transition-[background-color,box-shadow] duration-150 focus-within:bg-card focus-within:ring-ring/60"
				: "h-10 gap-3 rounded-md border border-border/40 bg-card/60 px-3 transition-colors duration-150 focus-within:ring-2 focus-within:ring-primary/30 focus-within:ring-offset-1 focus-within:ring-offset-background",
			disabled
				? "cursor-not-allowed opacity-50"
				: dense
					? "hover:bg-muted"
					: "hover:border-border/60 hover:bg-card/80",
			className,
		)}
	>
		{#snippet hexPart()}
			{#if showInput}
				<input
					bind:this={inputEl}
					type="text"
					spellcheck="false"
					autocomplete="off"
					class="h-6 w-[5.5rem] rounded-sm border border-primary/40 bg-background px-1.5 text-right font-mono text-[12px] font-medium uppercase tabular-nums text-foreground outline-none focus:border-primary/60 focus:ring-1 focus:ring-primary/30"
					value={inputValue}
					oninput={(e) =>
						(inputValue = (e.currentTarget as HTMLInputElement).value)}
					onkeydown={handleHexKeydown}
					onblur={commitHexInput}
					onclick={(e) => e.stopPropagation()}
					onpointerdown={(e) => e.stopPropagation()}
				/>
			{:else}
				<button
					type="button"
					{disabled}
					onmouseenter={() => (isHexHovered = true)}
					onmouseleave={() => (isHexHovered = false)}
					onclick={(e) => {
						if (!isHexEditable || disabled) return;
						e.stopPropagation();
						e.preventDefault();
						startHexEdit();
					}}
					onpointerdown={(e) => {
						if (isHexEditable) e.stopPropagation();
					}}
					aria-label={`${label} hex — hover and click to edit`}
					tabindex={isHexEditable ? 0 : -1}
					class={cn(
						"shrink-0 font-mono font-medium tabular-nums text-foreground/85 outline-none transition-colors",
						dense ? "text-[11px]" : "text-[12px]",
						isHexEditable &&
							"rounded-sm bg-foreground/[0.06] px-1 text-foreground cursor-text",
						!isHexEditable && "cursor-default",
					)}
					style:cursor={isHexEditable ? "text" : undefined}
				>
					{displayHex}
				</button>
			{/if}
		{/snippet}

		<!-- Swatch — opens the full ColorPicker popover. Checker grid sits
		     underneath so transparent/alpha colors read correctly. -->
		{#snippet swatchPart()}
			<Popover.Trigger>
				{#snippet child({ props })}
					<button
						type="button"
						{...props}
						{disabled}
						aria-label={`${label} swatch — opens color picker`}
						class={cn(
							"relative inline-block shrink-0 overflow-hidden rounded-md border border-border/60 shadow-[inset_0_0_0_1px_color-mix(in_srgb,_var(--color-foreground)_4%,_transparent)] outline-none transition-transform",
							"focus-visible:ring-2 focus-visible:ring-primary/30",
							dense ? "size-4" : "h-4 w-7",
							disabled
								? "cursor-not-allowed"
								: "cursor-pointer hover:scale-105 active:scale-95",
						)}
					>
						<span
							aria-hidden="true"
							class="absolute inset-0"
							style="background-image: conic-gradient(color-mix(in srgb, var(--color-foreground) 10%, transparent) 0deg 90deg, transparent 90deg 180deg, color-mix(in srgb, var(--color-foreground) 10%, transparent) 180deg 270deg, transparent 270deg 360deg); background-size: 6px 6px;"
						></span>
						<span
							aria-hidden="true"
							class="absolute inset-0"
							style:background={value}
						></span>
					</button>
				{/snippet}
			</Popover.Trigger>
		{/snippet}

		{#if hideLabel}
			{@render swatchPart()}
			{@render hexPart()}
			<div class="min-w-0 flex-1"></div>
		{:else}
			<Popover.Trigger>
				{#snippet child({ props })}
					<button
						type="button"
						{...props}
						{disabled}
						aria-label={`${label} — opens color picker`}
						class={cn(
							"flex min-w-0 flex-1 items-center gap-1.5 text-left outline-none",
							disabled ? "cursor-not-allowed" : "cursor-pointer",
						)}
					>
						{#if icon}
							<span
								class="flex size-3.5 shrink-0 items-center justify-center text-muted-foreground"
							>
								{@render icon()}
							</span>
						{/if}
						<span
							class="truncate text-[12px] font-medium text-muted-foreground"
						>
							{label}
						</span>
					</button>
				{/snippet}
			</Popover.Trigger>

			<div class="flex shrink-0 items-center gap-2">
				{@render hexPart()}
				{@render swatchPart()}
			</div>
		{/if}
	</div>

	<Popover.Content {align} sideOffset={6} class="w-auto p-0">
		<ColorPicker {value} {swatches} {recents} {allowAlpha} {oncommit} />
	</Popover.Content>
</Popover.Root>
