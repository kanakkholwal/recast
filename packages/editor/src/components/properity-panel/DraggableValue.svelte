<script lang="ts">
import { cn } from "@recast/ui/utils";
import {
	clampValue,
	DRAG_THRESHOLD_PX,
	dragDelta,
	formatValue,
	parseInputValue,
} from "./draggable-value.logic";

interface Props {
	label: string;
	value: number;
	min?: number;
	max?: number;
	/** Value change per px of drag and per arrow press. Shift ×10, Alt ×0.1. */
	step?: number;
	decimals?: number;
	suffix?: string;
	disabled?: boolean;
	/** Fired once when a drag passes the threshold; the place to push undo. */
	onDragStart?: () => void;
	/** Live value while dragging. Pair with an undo-suppressed write. */
	onInput?: (v: number) => void;
	/** Final value: drag end (`viaDrag`) or a typed/keyed edit. */
	onCommit: (v: number, viaDrag: boolean) => void;
	class?: string;
}

let {
	label,
	value,
	min,
	max,
	step = 1,
	decimals = 0,
	suffix,
	disabled = false,
	onDragStart,
	onInput,
	onCommit,
	class: className,
}: Props = $props();

let inputEl: HTMLInputElement | null = $state(null);
let dragging = $state(false);
// While the input holds focus it owns its text; outside of that the prop wins.
let editing = $state(false);
let draftText = $state("");

const display = $derived(dragging || editing ? draftText : formatValue(value, decimals));

function clamp(v: number) {
	return clampValue(v, min, max);
}

// --- drag-to-scrub on the label ---
let startX = 0;
let startValue = 0;
let engaged = false;

function onPointerDown(e: PointerEvent) {
	if (disabled || e.button !== 0) return;
	e.preventDefault();
	(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
	startX = e.clientX;
	startValue = value;
	engaged = false;
}

function onPointerMove(e: PointerEvent) {
	if (!(e.currentTarget as HTMLElement).hasPointerCapture?.(e.pointerId)) return;
	const dx = e.clientX - startX;
	if (!engaged) {
		if (Math.abs(dx) < DRAG_THRESHOLD_PX) return;
		engaged = true;
		dragging = true;
		onDragStart?.();
	}
	const next = clamp(startValue + dragDelta(dx, step, { coarse: e.shiftKey, fine: e.altKey }));
	draftText = formatValue(next, decimals);
	onInput?.(next);
}

function onPointerUp(e: PointerEvent) {
	(e.currentTarget as HTMLElement).releasePointerCapture?.(e.pointerId);
	if (engaged) {
		dragging = false;
		onCommit(parseInputValue(draftText, value), true);
	} else {
		// A plain click on the label hands focus to the field for typing.
		inputEl?.focus();
		inputEl?.select();
	}
	engaged = false;
}

function onPointerCancel() {
	if (engaged) {
		dragging = false;
		onCommit(parseInputValue(draftText, value), true);
	}
	engaged = false;
}

// --- typed edits ---
function onFocus() {
	editing = true;
	draftText = formatValue(value, decimals);
}

function commitTyped() {
	if (!editing) return;
	editing = false;
	const next = clamp(parseInputValue(draftText, value));
	if (next !== value) onCommit(next, false);
}

function onKeydown(e: KeyboardEvent) {
	if (e.key === "Enter") {
		e.preventDefault();
		commitTyped();
		inputEl?.blur();
	} else if (e.key === "Escape") {
		e.preventDefault();
		draftText = formatValue(value, decimals);
		editing = false;
		inputEl?.blur();
	} else if (e.key === "ArrowUp" || e.key === "ArrowDown") {
		e.preventDefault();
		const dir = e.key === "ArrowUp" ? 1 : -1;
		const next = clamp(value + dragDelta(dir, step, { coarse: e.shiftKey, fine: e.altKey }));
		editing = false;
		onCommit(next, false);
	}
}
</script>

<div
	class={cn(
		"flex h-7 min-w-0 items-center gap-1 rounded-md bg-muted/60 pl-2 pr-1.5 ring-1 ring-inset ring-border/40 transition-[box-shadow] focus-within:ring-ring/60",
		disabled && "pointer-events-none opacity-50",
		className,
	)}
>
	<span
		class="shrink-0 cursor-ew-resize touch-none select-none text-[10px] font-medium text-muted-foreground"
		onpointerdown={onPointerDown}
		onpointermove={onPointerMove}
		onpointerup={onPointerUp}
		onpointercancel={onPointerCancel}
		aria-hidden="true"
	>
		{label}
	</span>
	<input
		bind:this={inputEl}
		type="text"
		inputmode="decimal"
		aria-label={label}
		{disabled}
		value={display}
		oninput={(e) => (draftText = (e.currentTarget as HTMLInputElement).value)}
		onfocus={onFocus}
		onblur={commitTyped}
		onkeydown={onKeydown}
		class="w-full min-w-0 bg-transparent text-right font-mono text-[11px] tabular-nums text-foreground focus:outline-none"
	/>
	{#if suffix}
		<span class="shrink-0 text-[10px] text-muted-foreground/70" aria-hidden="true">{suffix}</span>
	{/if}
</div>
