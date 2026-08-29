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
	/** Show step/decile hash marks behind the fill. Default true. */
	hashMarks?: boolean;
	/** Compact h-8 variant on the shared field surface, for inline panel rows. */
	dense?: boolean;
	/** Drop the in-track label (keep the aria name) when an outer row labels it. */
	hideLabel?: boolean;
	onstart?: () => void;
	onchange?: (value: number) => void;
	oncommit?: (value: number) => void;
	formatValue?: (value: number, unit: string) => string;
}

function decimalsForStep(step: number): number {
	if (!Number.isFinite(step) || step <= 0) return 0;
	const str = step.toString();
	const idx = str.indexOf(".");
	return idx === -1 ? 0 : str.length - idx - 1;
}

function roundValue(v: number, step: number): number {
	return Number(v.toFixed(decimalsForStep(step)));
}

function snapToDecile(v: number, min: number, max: number): number {
	const range = max - min;
	if (range === 0) return min;
	const ratio = (v - min) / range;
	const decile = Math.round(ratio * 10) / 10;
	return min + decile * range;
}
</script>

<script lang="ts">
	import { tick } from "svelte";
	import { Spring } from "svelte/motion";
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
		hashMarks: showHashMarks = true,
		dense = false,
		hideLabel = false,
		onstart,
		onchange,
		oncommit,
		formatValue,
	}: SliderControlProps = $props();

	// `Row control` design — one card, label left, value right, animated fill
	// behind both. Click-anywhere snaps with a spring; drag scrubs in real
	// time; cursor past the track edges rubber-bands the row to telegraph the
	// clamp. Mirrors the geometry of <ColorField> so a stacked panel reads as
	// one form.

	const CLICK_THRESHOLD = 3;
	const DEAD_ZONE = 24;
	const MAX_CURSOR_RANGE = 200;
	const MAX_STRETCH = 8;
	const HANDLE_BUFFER = 8;
	const LABEL_CSS_LEFT = 12;
	const VALUE_CSS_RIGHT = 12;

	let wrapperEl: HTMLDivElement | null = $state(null);
	let trackEl: HTMLDivElement | null = $state(null);
	let labelEl: HTMLSpanElement | null = $state(null);
	let valueEl: HTMLSpanElement | null = $state(null);
	let inputEl: HTMLInputElement | null = $state(null);

	let isInteracting = $state(false);
	let isDragging = $state(false);
	let isHovered = $state(false);
	let isValueHovered = $state(false);
	// The value is ALWAYS typeable. It used to unlock only after an 800ms hover,
	// which meant keyboard and touch users could never enter an exact number into
	// any of the ~45 sliders in the editor -- in a precision tool.
	const isValueEditable = $derived(!disabled);
	let showInput = $state(false);
	let inputValue = $state("");

	const precision = $derived(decimalsForStep(step));
	// svelte-ignore state_referenced_locally
	const initialPercent = ((value - min) / Math.max(max - min, 1e-9)) * 100;

	// Spring-driven motion. Drag scrubs commit instant (no animation), and
	// click-snap / release commits animate. Rubber-band stretches the track
	// outward when the cursor pulls past either edge. Handle scale/opacity
	// dodge when the thumb position would collide with label/value text.
	const fillPercent = new Spring(initialPercent, {
		stiffness: 0.25,
		damping: 0.7,
	});
	const rubberStretchPx = new Spring(0, { stiffness: 0.2, damping: 0.65 });
	const handleOpacityMv = new Spring(0, { stiffness: 0.3, damping: 0.75 });
	const handleScaleXMv = new Spring(0.25, { stiffness: 0.2, damping: 0.6 });
	const handleScaleYMv = new Spring(1, { stiffness: 0.2, damping: 0.6 });

	let pointerDownPos: { x: number; y: number } | null = null;
	let isClick = true;
	let trackRect: DOMRect | null = null;
	let trackScale = 1;

	function percentFromValue(v: number) {
		if (max <= min) return 0;
		return ((v - min) / (max - min)) * 100;
	}

	function clampValue(v: number) {
		if (max <= min) return min;
		return Math.max(min, Math.min(max, v));
	}

	function normalizeValue(v: number) {
		if (max <= min) return min;
		const safeStep = Number.isFinite(step) && step > 0 ? step : 1;
		const stepped = min + Math.round((v - min) / safeStep) * safeStep;
		return roundValue(clampValue(stepped), safeStep);
	}

	function positionToValue(clientX: number) {
		if (!trackRect || !trackEl) return value;
		const localX = clientX - trackRect.left;
		const sceneX = localX / trackScale;
		const width = trackEl.offsetWidth || trackRect.width || 1;
		const ratio = Math.max(0, Math.min(1, sceneX / width));
		return clampValue(min + ratio * (max - min));
	}

	function computeRubberStretch(clientX: number, sign: -1 | 1) {
		if (!trackRect) return 0;
		const distancePast =
			sign < 0 ? trackRect.left - clientX : clientX - trackRect.right;
		const overflow = Math.max(0, distancePast - DEAD_ZONE);
		return (
			sign *
			MAX_STRETCH *
			Math.sqrt(Math.min(overflow / MAX_CURSOR_RANGE, 1))
		);
	}

	function defaultFormatValue(v: number, u: string) {
		const formatted =
			precision > 0 ? v.toFixed(precision) : `${Math.round(v)}`;
		return `${formatted}${u}`;
	}

	const formattedValue = $derived(
		(formatValue ?? defaultFormatValue)(value, unit),
	);
	const editDisplayValue = $derived(
		precision > 0 ? value.toFixed(precision) : `${Math.round(value)}`,
	);

	const percentage = $derived(percentFromValue(value));
	const isActive = $derived(isInteracting || isHovered);

	// When the thumb would overlap the label or value text, dim+squish it so
	// the row's typography stays the visual priority.
	const leftThreshold = $derived.by(() => {
		const w = trackEl?.offsetWidth;
		if (w && labelEl) {
			return (
				((LABEL_CSS_LEFT + labelEl.offsetWidth + HANDLE_BUFFER) / w) * 100
			);
		}
		return 30;
	});
	const rightThreshold = $derived.by(() => {
		const w = trackEl?.offsetWidth;
		if (w && valueEl) {
			return (
				((w - VALUE_CSS_RIGHT - valueEl.offsetWidth - HANDLE_BUFFER) / w) *
				100
			);
		}
		return 78;
	});
	const valueDodge = $derived(
		percentage < leftThreshold || percentage > rightThreshold,
	);

	const handleOpacity = $derived.by(() => {
		if (!isActive) return 0;
		if (valueDodge) return 0.12;
		if (isDragging) return 0.95;
		return 0.6;
	});

	$effect(() => {
		// Keep fill spring in sync with external value changes (without
		// re-animating during drag).
		const p = percentFromValue(value);
		if (!isInteracting) {
			fillPercent.set(p, { instant: false });
		}
	});

	$effect(() => {
		handleOpacityMv.set(handleOpacity);
		handleScaleXMv.set(isActive ? 1 : 0.25);
		handleScaleYMv.set(isActive && valueDodge ? 0.7 : 1);
	});

	$effect(() => {
		if (showInput) {
			tick().then(() => {
				inputEl?.focus();
				inputEl?.select();
			});
		}
	});

	// Hash marks. When discrete steps are sparse, show one per step; otherwise
	// fall back to ten decile marks for visual rhythm.
	const discreteSteps = $derived((max - min) / Math.max(step, 1e-9));
	const marks = $derived.by(() => {
		if (!showHashMarks) return [];
		if (!Number.isFinite(discreteSteps) || discreteSteps <= 1) return [];
		if (discreteSteps <= 10) {
			return Array.from(
				{ length: Math.max(Math.floor(discreteSteps) - 1, 0) },
				(_, i) => ({
					key: `d-${i + 1}`,
					left: ((i + 1) * step) / (max - min) * 100,
				}),
			);
		}
		return Array.from({ length: 9 }, (_, i) => ({
			key: `t-${i + 1}`,
			left: (i + 1) * 10,
		}));
	});

	function commitValue(next: number, fireCommit = false) {
		const normalized = normalizeValue(next);
		const changed = normalized !== value;
		if (changed) {
			value = normalized;
			onchange?.(normalized);
		}
		if (fireCommit) {
			oncommit?.(normalized);
		}
	}

	function handlePointerDown(e: PointerEvent) {
		if (disabled || showInput) return;
		if (e.button !== undefined && e.button !== 0) return;
		// Don't hijack pointerdown that started on the editable value span.
		if (
			isValueEditable &&
			valueEl &&
			e.target instanceof Node &&
			valueEl.contains(e.target)
		) {
			return;
		}
		e.preventDefault();

		(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
		pointerDownPos = { x: e.clientX, y: e.clientY };
		isClick = true;
		isInteracting = true;
		onstart?.();

		if (trackEl) {
			trackRect = trackEl.getBoundingClientRect();
			trackScale = trackRect.width / (trackEl.offsetWidth || trackRect.width);
		}
	}

	function handlePointerMove(e: PointerEvent) {
		if (!isInteracting || !pointerDownPos) return;

		const dx = e.clientX - pointerDownPos.x;
		const dy = e.clientY - pointerDownPos.y;
		const distance = Math.hypot(dx, dy);

		if (isClick && distance > CLICK_THRESHOLD) {
			isClick = false;
			isDragging = true;
		}

		if (!isClick) {
			if (trackRect) {
				if (e.clientX < trackRect.left) {
					rubberStretchPx.set(computeRubberStretch(e.clientX, -1), {
						instant: true,
					});
				} else if (e.clientX > trackRect.right) {
					rubberStretchPx.set(computeRubberStretch(e.clientX, 1), {
						instant: true,
					});
				} else {
					rubberStretchPx.set(0, { instant: true });
				}
			}

			const next = positionToValue(e.clientX);
			fillPercent.set(percentFromValue(next), { instant: true });
			commitValue(next);
		}
	}

	function handlePointerUp(e: PointerEvent) {
		if (!isInteracting) return;

		if (isClick) {
			// Click-snap: spring-animate the fill to the clicked position.
			const raw = positionToValue(e.clientX);
			const snapped =
				discreteSteps <= 10
					? normalizeValue(raw)
					: roundValue(snapToDecile(raw, min, max), step);
			fillPercent.set(percentFromValue(snapped));
			commitValue(snapped, true);
		} else {
			oncommit?.(normalizeValue(value));
		}

		if (rubberStretchPx.current !== 0) {
			rubberStretchPx.set(0);
		}

		isInteracting = false;
		isDragging = false;
		pointerDownPos = null;
	}

	function handlePointerCancel() {
		if (!isInteracting) return;
		isInteracting = false;
		isDragging = false;
		rubberStretchPx.set(0, { instant: true });
		pointerDownPos = null;
	}

	function handleKeydown(e: KeyboardEvent) {
		if (disabled || showInput) return;
		const pageStep = step * 10;
		switch (e.key) {
			case "ArrowLeft":
			case "ArrowDown":
				e.preventDefault();
				onstart?.();
				commitValue(value - step, true);
				break;
			case "ArrowRight":
			case "ArrowUp":
				e.preventDefault();
				onstart?.();
				commitValue(value + step, true);
				break;
			case "PageDown":
				e.preventDefault();
				onstart?.();
				commitValue(value - pageStep, true);
				break;
			case "PageUp":
				e.preventDefault();
				onstart?.();
				commitValue(value + pageStep, true);
				break;
			case "Home":
				e.preventDefault();
				onstart?.();
				commitValue(min, true);
				break;
			case "End":
				e.preventDefault();
				onstart?.();
				commitValue(max, true);
				break;
		}
	}

	function handleValueClick(e: MouseEvent) {
		if (!isValueEditable || disabled) return;
		e.stopPropagation();
		e.preventDefault();
		onstart?.();
		showInput = true;
		inputValue = editDisplayValue;
	}

	function commitInput() {
		const parsed = Number.parseFloat(inputValue);
		if (Number.isFinite(parsed)) {
			commitValue(clampValue(parsed), true);
		}
		showInput = false;
		isValueHovered = false;
	}

	function handleInputKeydown(e: KeyboardEvent) {
		if (e.key === "Enter") {
			e.preventDefault();
			commitInput();
		} else if (e.key === "Escape") {
			e.preventDefault();
			showInput = false;
			isValueHovered = false;
		}
	}

	// Inline transform/width strings — bound to spring `.current` for
	// reactivity. The rubber-band shifts the *track* (not the wrapper) so the
	// row's outline doesn't visibly move; only the fillable region stretches.
	const trackStyle = $derived(
		`width: calc(100% + ${Math.abs(rubberStretchPx.current)}px); transform: translateX(${rubberStretchPx.current < 0 ? rubberStretchPx.current : 0}px);`,
	);
</script>

<div
	bind:this={wrapperEl}
	role="group"
	aria-label={label}
	class={cn(
		"relative w-full select-none overflow-hidden outline-none transition-colors duration-150",
		dense
			? "h-8 rounded-lg bg-muted/60 ring-1 ring-inset ring-border/40 focus-within:ring-ring/60"
			: "h-10 rounded-md border border-border/40 bg-card/60 focus-within:ring-2 focus-within:ring-primary/30 focus-within:ring-offset-1 focus-within:ring-offset-background",
		disabled
			? "cursor-not-allowed opacity-50"
			: dense
				? "hover:bg-muted"
				: "hover:border-border/60 hover:bg-card/80",
		className,
	)}
	onmouseenter={() => (isHovered = true)}
	onmouseleave={() => (isHovered = false)}
>
	<div
		bind:this={trackEl}
		class={cn(
			"group/slider relative flex h-full items-center px-3",
			disabled ? "cursor-not-allowed" : "cursor-ew-resize",
		)}
		style={trackStyle}
		onpointerdown={handlePointerDown}
		onpointermove={handlePointerMove}
		onpointerup={handlePointerUp}
		onpointercancel={handlePointerCancel}
		onlostpointercapture={handlePointerCancel}
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
		<!-- Hash marks. Sit at z-0 behind the fill. -->
		{#if marks.length}
			<div class="pointer-events-none absolute inset-y-2 left-1 right-1 z-0">
				{#each marks as mark (mark.key)}
					<span
						class="absolute top-1/2 h-1 w-px -translate-y-1/2 rounded-full bg-foreground/10"
						style:left={`${mark.left}%`}
					></span>
				{/each}
			</div>
		{/if}

		<!-- Spring-animated fill. -->
		<div
			class="pointer-events-none absolute inset-y-1 left-1 z-0 rounded-[5px] bg-foreground/[0.07]"
			style:width={fillPercent.current > 0
				? `max(calc(${fillPercent.current}% - 8px), 0.5rem)`
				: "0px"}
		></div>

		<!-- Pill thumb. Vertically positioned with symmetric inset so the
		     transform stack only carries scale (origin: center) and stays
		     glued to the row's midline regardless of scaleY. -->
		<div
			class={cn(
				"pointer-events-none absolute inset-y-[21%] z-10 w-[3px] rounded-full bg-primary shadow-[0_0_0_1px_color-mix(in_srgb,_var(--color-background)_50%,_transparent)]",
			)}
			style:left={`max(5px, calc(${fillPercent.current}% - 1.5px))`}
			style:opacity={handleOpacityMv.current}
			style:transform-origin="center"
			style:transform={`scaleX(${handleScaleXMv.current}) scaleY(${handleScaleYMv.current})`}
		></div>

		<!-- Label (left). z-20 so it floats above the fill. Hidden when an outer
		     row already labels the slider; the flex-1 spacer stays to keep the
		     value pinned right. -->
		<div class="pointer-events-none relative z-20 flex min-w-0 flex-1 items-center gap-1.5">
			{#if icon}
				<span
					class="flex size-3.5 shrink-0 items-center justify-center text-muted-foreground"
				>
					{@render icon()}
				</span>
			{/if}
			{#if !hideLabel}
				<span
					bind:this={labelEl}
					class={cn("truncate font-medium text-muted-foreground", dense ? "text-[11px]" : "text-[12px]")}
				>
					{label}
				</span>
			{/if}
		</div>

		<!-- Value (right). Click, or focus and press Enter, to type an exact value. -->
		{#if showInput}
			<input
				bind:this={inputEl}
				type="text"
				inputmode="decimal"
				class="relative z-20 ml-3 h-6 w-16 shrink-0 rounded-sm border border-primary/40 bg-background px-1.5 text-right font-mono text-[12px] font-medium tabular-nums text-foreground outline-none focus:border-primary/60 focus:ring-1 focus:ring-primary/30"
				value={inputValue}
				oninput={(e) =>
					(inputValue = (e.currentTarget as HTMLInputElement).value)}
				onkeydown={handleInputKeydown}
				onblur={commitInput}
				onclick={(e) => e.stopPropagation()}
				onpointerdown={(e) => e.stopPropagation()}
				onmousedown={(e) => e.stopPropagation()}
			/>
		{:else}
			<span
				bind:this={valueEl}
				role="button"
				tabindex={isValueEditable ? 0 : -1}
				aria-label={isValueEditable
					? `${label}: type an exact value`
					: undefined}
				class={cn(
					"relative z-20 shrink-0 pl-3 font-mono font-medium tabular-nums text-foreground/85 transition-colors",
					dense ? "text-[11px]" : "text-[12px]",
					"focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-ring",
					isValueEditable &&
						isValueHovered &&
						"rounded-sm bg-foreground/[0.06] px-1 text-foreground",
				)}
				onmouseenter={() => (isValueHovered = true)}
				onmouseleave={() => (isValueHovered = false)}
				onclick={handleValueClick}
				onkeydown={(e) => {
					if (isValueEditable && (e.key === "Enter" || e.key === " ")) {
						e.preventDefault();
						e.stopPropagation();
						onstart?.();
						showInput = true;
						inputValue = editDisplayValue;
					}
				}}
				onpointerdown={(e) => {
					if (isValueEditable) e.stopPropagation();
				}}
				onmousedown={(e) => {
					if (isValueEditable) e.stopPropagation();
				}}
				style:cursor={isValueEditable ? "text" : "inherit"}
			>
				{formattedValue}
			</span>
		{/if}
	</div>
</div>
