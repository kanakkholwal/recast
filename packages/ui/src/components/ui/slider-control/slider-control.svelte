<script lang="ts" module>
	import type { Snippet } from "svelte";

	export interface SliderControlProps {
		label: string;
		value: number;
		min?: number;
		max?: number;
		step?: number;
		/** Visible only via the `title` tooltip — no extra row height. */
		description?: string;
		/** Leading glyph rendered next to the label. */
		icon?: Snippet;
		/** Appended to the formatted value when no `formatValue` is given. */
		unit?: string;
		disabled?: boolean;
		class?: string;
		onstart?: () => void;
		onchange?: (value: number) => void;
		oncommit?: (value: number) => void;
		formatValue?: (value: number, unit: string) => string;
	}
</script>

<script lang="ts">
	import { cn } from "@recast/ui/utils";

	let {
		label,
		value = $bindable(),
		min = 0,
		max = 100,
		step = 1,
		description,
		icon,
		unit = "",
		disabled = false,
		class: className,
		onstart,
		onchange,
		oncommit,
		formatValue,
	}: SliderControlProps = $props();

	// `Row control` design — one card, label left, value right, draggable
	// fill behind both. Mirrors the visual rhythm of <ColorField>, so a
	// panel that stacks slider + color rows reads as a consistent form.

	let trackEl: HTMLDivElement | null = $state(null);
	let isDragging = $state(false);
	let activePointerId = $state<number | null>(null);

	function getStepPrecision(input: number) {
		if (!Number.isFinite(input)) return 0;
		const parts = input.toString().split(".");
		return parts[1]?.length ?? 0;
	}

	const precision = $derived(getStepPrecision(step));
	const percentage = $derived.by(() => {
		if (max <= min) return 0;
		return ((value - min) / (max - min)) * 100;
	});
	const clampedPercentage = $derived(
		Math.min(100, Math.max(0, percentage)),
	);

	function defaultFormatValue(nextValue: number, nextUnit: string) {
		const formatted =
			precision > 0 ? nextValue.toFixed(precision) : `${Math.round(nextValue)}`;
		return `${formatted}${nextUnit}`;
	}

	const formattedValue = $derived(
		(formatValue ?? defaultFormatValue)(value, unit),
	);

	function normalizeValue(nextValue: number) {
		if (max <= min) return min;
		const safeStep = Number.isFinite(step) && step > 0 ? step : 1;
		const stepped = min + Math.round((nextValue - min) / safeStep) * safeStep;
		const clamped = Math.min(max, Math.max(min, stepped));
		return Number(clamped.toFixed(precision));
	}

	function commitValue(nextValue: number, shouldCommit = false) {
		const normalized = normalizeValue(nextValue);
		const changed = normalized !== value;

		if (changed) {
			value = normalized;
			onchange?.(normalized);
		}

		if (shouldCommit) {
			oncommit?.(normalized);
		}
	}

	function updateFromPointer(event: PointerEvent, shouldCommit = false) {
		if (!trackEl || disabled) return;
		const rect = trackEl.getBoundingClientRect();
		if (rect.width <= 0) return;

		const offsetX = Math.min(
			Math.max(event.clientX - rect.left, 0),
			rect.width,
		);
		const ratio = offsetX / rect.width;
		const nextValue = min + ratio * (max - min);
		commitValue(nextValue, shouldCommit);
	}

	function handlePointerDown(event: PointerEvent) {
		if (disabled || !trackEl) return;
		event.preventDefault();
		onstart?.();
		isDragging = true;
		activePointerId = event.pointerId;
		trackEl.setPointerCapture(event.pointerId);
		updateFromPointer(event);
	}

	function handlePointerMove(event: PointerEvent) {
		if (!isDragging || disabled) return;
		if (activePointerId !== null && event.pointerId !== activePointerId) return;
		updateFromPointer(event);
	}

	function finishPointerInteraction(event?: PointerEvent) {
		if (!isDragging) return;
		if (
			event &&
			activePointerId !== null &&
			event.pointerId !== activePointerId
		) {
			return;
		}

		if (event) {
			updateFromPointer(event, true);
		} else {
			oncommit?.(normalizeValue(value));
		}

		isDragging = false;
		activePointerId = null;
	}

	function handleKeydown(event: KeyboardEvent) {
		if (disabled) return;

		const pageStep = step * 10;
		switch (event.key) {
			case "ArrowLeft":
			case "ArrowDown":
				event.preventDefault();
				onstart?.();
				commitValue(value - step, true);
				break;
			case "ArrowRight":
			case "ArrowUp":
				event.preventDefault();
				onstart?.();
				commitValue(value + step, true);
				break;
			case "PageDown":
				event.preventDefault();
				onstart?.();
				commitValue(value - pageStep, true);
				break;
			case "PageUp":
				event.preventDefault();
				onstart?.();
				commitValue(value + pageStep, true);
				break;
			case "Home":
				event.preventDefault();
				onstart?.();
				commitValue(min, true);
				break;
			case "End":
				event.preventDefault();
				onstart?.();
				commitValue(max, true);
				break;
		}
	}
</script>

<div
	bind:this={trackEl}
	class={cn(
		"group/slider relative flex h-10 w-full select-none items-center overflow-hidden rounded-md border border-border/40 bg-card/60 px-3 outline-none transition-colors duration-150",
		"focus-visible:ring-2 focus-visible:ring-primary/30 focus-visible:ring-offset-1 focus-visible:ring-offset-background",
		disabled
			? "cursor-not-allowed opacity-50"
			: "cursor-ew-resize hover:border-border/60 hover:bg-card/80",
		className,
	)}
	onpointerdown={handlePointerDown}
	onpointermove={handlePointerMove}
	onpointerup={finishPointerInteraction}
	onpointercancel={finishPointerInteraction}
	onlostpointercapture={() => finishPointerInteraction()}
	onkeydown={handleKeydown}
	role="slider"
	tabindex={disabled ? -1 : 0}
	aria-disabled={disabled}
	aria-valuenow={value}
	aria-valuemin={min}
	aria-valuemax={max}
	aria-valuetext={formattedValue}
	aria-label={label}
	title={description}
>
	<!-- Fill behind the thumb. Sits behind everything else (z-0) so the
	     label/value layer reads as if it's printed on top of the same card. -->
	<div
		class="pointer-events-none absolute inset-y-1 left-1 rounded-[5px] bg-foreground/[0.07]"
		style:width={clampedPercentage > 0
			? `max(calc(${clampedPercentage}% - 8px), 0.5rem)`
			: "0px"}
	></div>

	<!-- Pill-style thumb. Slightly wider than a hairline so it reads as a
	     handle on hover/drag without crowding the row's typography. -->
	<div
		class={cn(
			"pointer-events-none absolute inset-y-[22%] z-10 w-1 rounded-full bg-primary/55 shadow-[0_0_0_1px_color-mix(in_srgb,_var(--color-background)_45%,_transparent)] transition-[transform,background] duration-150",
			"group-hover/slider:bg-primary/80",
			isDragging && "scale-y-110 bg-primary",
		)}
		style:left={`calc(${clampedPercentage}% - 4px)`}
	></div>

	<div class="pointer-events-none relative z-20 flex min-w-0 flex-1 items-center gap-1.5">
		{#if icon}
			<span class="flex size-3.5 shrink-0 items-center justify-center text-muted-foreground">
				{@render icon()}
			</span>
		{/if}
		<span class="truncate text-[12px] font-medium text-muted-foreground">
			{label}
		</span>
	</div>

	<span
		class="pointer-events-none relative z-20 shrink-0 pl-3 font-mono text-[12px] font-medium tabular-nums text-foreground/85"
	>
		{formattedValue}
	</span>
</div>
