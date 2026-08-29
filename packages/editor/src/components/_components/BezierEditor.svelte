<script lang="ts">
import { cn } from "@recast/ui/utils";
import {
	clampEasingCoord,
	EASING_OVERSHOOT,
	type Easing,
	sampleCurve,
} from "../../lib/easing/cubic-bezier";
import DraggableValue from "../properity-panel/DraggableValue.svelte";
import PropRow from "../properity-panel/PropRow.svelte";

interface Props {
	value: Easing;
	onchange: (next: Easing) => void;
	label?: string;
	description?: string;
	/** Graph size in px (square). Padding for overshoot is added around it. */
	size?: number;
	disabled?: boolean;
}

let { value, onchange, label, description, size = 176, disabled = false }: Props = $props();

// viewBox is the unit square plus the overshoot band so bounce/spring handles
// (y outside [0,1]) stay grabbable. y is flipped at render time (SVG y grows down).
const VB_MIN = -EASING_OVERSHOOT;
const VB_SPAN = 1 + EASING_OVERSHOOT * 2;

// Arrow-key step, and the coarse step for Shift+Arrow.
const KEY_STEP = 0.01;
const KEY_STEP_COARSE = 0.1;

let svgEl: SVGSVGElement | null = $state(null);
let dragging: "p1" | "p2" | null = $state(null);
let activePointerId = $state<number | null>(null);

const curvePath = $derived.by(() => {
	const pts = sampleCurve(value, 48);
	return pts
		.map(([x, y], i) => `${i === 0 ? "M" : "L"} ${x.toFixed(4)} ${(1 - y).toFixed(4)}`)
		.join(" ");
});

function svgPoint(e: PointerEvent): { x: number; y: number } | null {
	if (!svgEl) return null;
	const pt = svgEl.createSVGPoint();
	pt.x = e.clientX;
	pt.y = e.clientY;
	const ctm = svgEl.getScreenCTM();
	if (!ctm) return null;
	const { x, y } = pt.matrixTransform(ctm.inverse());
	return { x, y: 1 - y };
}

function updateHandle(which: "p1" | "p2", x: number, y: number) {
	if (which === "p1") {
		onchange({
			...value,
			x1: clampEasingCoord("x1", x),
			y1: clampEasingCoord("y1", y),
		});
	} else {
		onchange({
			...value,
			x2: clampEasingCoord("x2", x),
			y2: clampEasingCoord("y2", y),
		});
	}
}

// The handles carry `role="slider"` and were focusable, but nothing listened
// for keys: a keyboard user heard "Control point 1, slider" and could not move
// it. Left/Right walk x, Up/Down walk y, Shift for a coarse step.
function handleKey(which: "p1" | "p2", e: KeyboardEvent) {
	if (disabled) return;
	const step = e.shiftKey ? KEY_STEP_COARSE : KEY_STEP;
	const x = which === "p1" ? value.x1 : value.x2;
	const y = which === "p1" ? value.y1 : value.y2;
	switch (e.key) {
		case "ArrowLeft":
			updateHandle(which, x - step, y);
			break;
		case "ArrowRight":
			updateHandle(which, x + step, y);
			break;
		case "ArrowDown":
			updateHandle(which, x, y - step);
			break;
		case "ArrowUp":
			updateHandle(which, x, y + step);
			break;
		case "Home":
			updateHandle(which, 0, y);
			break;
		case "End":
			updateHandle(which, 1, y);
			break;
		default:
			return;
	}
	e.preventDefault();
}

function handleStart(which: "p1" | "p2", e: PointerEvent) {
	if (disabled) return;
	e.preventDefault();
	dragging = which;
	activePointerId = e.pointerId;
	(e.currentTarget as Element).setPointerCapture(e.pointerId);
}

function handleMove(e: PointerEvent) {
	if (!dragging || e.pointerId !== activePointerId) return;
	const p = svgPoint(e);
	if (!p) return;
	updateHandle(dragging, p.x, p.y);
}

function handleEnd(e: PointerEvent) {
	if (!dragging || e.pointerId !== activePointerId) return;
	(e.currentTarget as Element).releasePointerCapture(e.pointerId);
	dragging = null;
	activePointerId = null;
}

// Dragging clamped y; typing did not, so `y1: 50` parked the handle far
// outside the viewBox where no pointer could reach it again.
function setField(field: keyof Easing, n: number) {
	onchange({ ...value, [field]: clampEasingCoord(field, n) });
}

function fieldBounds(field: keyof Easing) {
	return field === "x1" || field === "x2"
		? { min: 0, max: 1 }
		: { min: -EASING_OVERSHOOT, max: 1 + EASING_OVERSHOOT };
}
</script>

<div class="flex flex-col gap-2">
  {#if label}
    <div class="flex items-baseline justify-between">
      <span class="text-[11px] font-medium text-foreground">{label}</span>
      {#if description}
        <span class="text-[10px] text-muted-foreground">{description}</span>
      {/if}
    </div>
  {/if}

  <div
    class={cn(
      "relative rounded-md border border-border bg-card/50",
      disabled && "pointer-events-none opacity-60",
    )}
    style:padding="6px"
  >
    <!-- Surface only receives pointermove/up to continue a drag; the slider roles live on the handle circles. -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <svg
      bind:this={svgEl}
      viewBox="{VB_MIN} {VB_MIN} {VB_SPAN} {VB_SPAN}"
      preserveAspectRatio="xMidYMid meet"
      width={size}
      height={size}
      aria-label="Cubic-bezier curve editor"
      class="block w-full cursor-default select-none touch-none"
      onpointermove={handleMove}
      onpointerup={handleEnd}
      onpointercancel={handleEnd}
    >
      <!-- Grid -->
      <g stroke="currentColor" stroke-width="0.003" class="text-border">
        <line x1="0" y1="0" x2="1" y2="0" />
        <line x1="0" y1="1" x2="1" y2="1" />
        <line x1="0" y1="0" x2="0" y2="1" />
        <line x1="1" y1="0" x2="1" y2="1" />
        <line
          x1="0"
          y1="0.5"
          x2="1"
          y2="0.5"
          stroke-dasharray="0.01 0.01"
          stroke-opacity="0.5"
        />
        <line
          x1="0.5"
          y1="0"
          x2="0.5"
          y2="1"
          stroke-dasharray="0.01 0.01"
          stroke-opacity="0.5"
        />
      </g>

      <!-- Axis labels (tiny) -->
      <g
        class="text-muted-foreground"
        fill="currentColor"
        font-size="0.06"
        font-family="ui-monospace, monospace"
      >
        <text x="-0.03" y="1.06" text-anchor="end">0</text>
        <text x="-0.03" y="0.02" text-anchor="end">1</text>
        <text x="0" y="1.14" text-anchor="start">0</text>
        <text x="1" y="1.14" text-anchor="end">1</text>
      </g>

      <!-- Tangent lines from anchors to control points -->
      <g
        stroke="currentColor"
        stroke-width="0.004"
        class="text-muted-foreground"
        opacity="0.6"
      >
        <line x1="0" y1="1" x2={value.x1} y2={1 - value.y1} />
        <line x1="1" y1="0" x2={value.x2} y2={1 - value.y2} />
      </g>

      <!-- Curve -->
      <path
        d={curvePath}
        stroke="currentColor"
        class="text-foreground"
        stroke-width="0.012"
        fill="none"
      />

      <!-- Anchor points (non-interactive) -->
      <circle
        cx="0"
        cy="1"
        r="0.018"
        fill="currentColor"
        class="text-muted-foreground"
      />
      <circle
        cx="1"
        cy="0"
        r="0.018"
        fill="currentColor"
        class="text-muted-foreground"
      />

      <!-- Handles: an invisible, larger hit circle carries the grab (the visible
           dot alone is a ~6px Fitts target); the visible circle keeps the slider
           role and keyboard support. -->
      {#each [
        ["p1", value.x1, value.y1],
        ["p2", value.x2, value.y2],
      ] as const as [which, hx, hy] (which)}
        <circle
          cx={hx}
          cy={1 - hy}
          r="0.075"
          fill="transparent"
          aria-hidden="true"
          class={cn(!disabled && "cursor-grab")}
          style:cursor={dragging === which ? "grabbing" : undefined}
          onpointerdown={(e) => handleStart(which, e)}
        />
        <circle
          cx={hx}
          cy={1 - hy}
          r={dragging === which ? 0.042 : 0.034}
          fill="currentColor"
          stroke="var(--color-background)"
          stroke-width="0.008"
          role="slider"
          tabindex="0"
          aria-label={which === "p1" ? "Control point 1" : "Control point 2"}
          aria-valuemin={0}
          aria-valuemax={1}
          aria-valuenow={hx}
          aria-valuetext="x {hx.toFixed(2)}, y {hy.toFixed(2)}"
          class={cn("text-foreground focus:outline-none", !disabled && "cursor-grab")}
          style:cursor={dragging === which ? "grabbing" : undefined}
          onpointerdown={(e) => handleStart(which, e)}
          onkeydown={(e) => handleKey(which, e)}
        />
      {/each}
    </svg>
  </div>

  <!-- Drag-to-scrub value fields (drag the axis label; click to type). Paired
       per control point, row-labelled like the reference transform inspector. -->
  {#each [
    ["Start", "x1", "y1"],
    ["End", "x2", "y2"],
  ] as const as [rowLabel, xf, yf] (rowLabel)}
    <PropRow label={rowLabel}>
      <DraggableValue
        class="flex-1"
        label="X"
        value={value[xf]}
        min={fieldBounds(xf).min}
        max={fieldBounds(xf).max}
        step={0.01}
        decimals={2}
        {disabled}
        onInput={(n) => setField(xf, n)}
        onCommit={(n) => setField(xf, n)}
      />
      <DraggableValue
        class="flex-1"
        label="Y"
        value={value[yf]}
        min={fieldBounds(yf).min}
        max={fieldBounds(yf).max}
        step={0.01}
        decimals={2}
        {disabled}
        onInput={(n) => setField(yf, n)}
        onCommit={(n) => setField(yf, n)}
      />
    </PropRow>
  {/each}
</div>
