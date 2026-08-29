<script lang="ts">
import type { IconComponent } from "@recast/icons";
import { cn } from "@recast/ui/utils";
import type { Snippet } from "svelte";
import {
	clampValue,
	DRAG_THRESHOLD_PX,
	dragDelta,
	formatValue,
	parseInputValue,
} from "./draggable-value.logic";

// Figma/Premiere-grade numeric field
interface Props {
	label: string;
	/** Leading glyph: a short string ("X", "W") or a snippet for custom content. */
	glyph?: string | Snippet;
	/** Leading property icon; doubles as the drag-scrub handle. Wins over `glyph`. */
	icon?: IconComponent;
	value: number;
	min?: number;
	max?: number;
	/** Value change per px of drag and per arrow press. Shift ×10, Alt ×0.1. */
	step?: number;
	decimals?: number;
	suffix?: string;
	disabled?: boolean;
	onDragStart?: () => void;
	onInput?: (v: number) => void;
	onCommit: (v: number, viaDrag: boolean) => void;
	class?: string;
}

let {
	label,
	glyph,
	icon: Icon,
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
let editing = $state(false);
let draftText = $state("");

// Reads like the reference inspectors instead of floating at the field's right edge.
const display = $derived(
	editing ? draftText : `${dragging ? draftText : formatValue(value, decimals)}${suffix ?? ""}`,
);
const glyphIsText = $derived(typeof glyph === "string");

function clamp(v: number) {
	return clampValue(v, min, max);
}

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
		"flex h-8 min-w-0 items-center gap-1.5 rounded-lg bg-muted/60 pl-2.5 pr-2 ring-1 ring-inset ring-border/40 transition-[background-color,box-shadow] hover:bg-muted focus-within:bg-card focus-within:ring-ring/60",
		disabled && "pointer-events-none opacity-50",
		className,
	)}
>
	{#if Icon || glyph !== undefined}
		<span
			class="flex shrink-0 cursor-ew-resize touch-none select-none items-center justify-center text-muted-foreground/80"
			onpointerdown={onPointerDown}
			onpointermove={onPointerMove}
			onpointerup={onPointerUp}
			onpointercancel={onPointerCancel}
			aria-hidden="true"
		>
			{#if Icon}
				<Icon class="size-3" />
			{:else if glyphIsText}
				<span class="text-[10px] font-semibold uppercase">{glyph}</span>
			{:else}
				{@render (glyph as Snippet)()}
			{/if}
		</span>
	{/if}
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
		class="w-full min-w-0 bg-transparent font-mono text-[11px] tabular-nums text-foreground focus:outline-none"
	/>
</div>
