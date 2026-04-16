<script lang="ts">
  import {
    EASING_PRESETS,
    easingEquals,
    sampleCurve,
    type Easing,
  } from "$lib/easing/cubic-bezier";
  import { Input } from "@recast/ui/input";
  import { cn } from "@recast/ui/utils";

  interface Props {
    value: Easing;
    onchange: (next: Easing) => void;
    label?: string;
    description?: string;
    /** Hide preset chips when the editor is used in a compact context. */
    showPresets?: boolean;
    /** Graph size in px (square). Padding for overshoot is added around it. */
    size?: number;
    disabled?: boolean;
  }

  let {
    value,
    onchange,
    label,
    description,
    showPresets = true,
    size = 176,
    disabled = false,
  }: Props = $props();

  // The SVG viewBox is the unit square `0..1`, with `overshoot` units of
  // padding on each side so handles whose y exits [0,1] (bounce / spring)
  // remain grabbable. We flip y at render time because SVG y grows down.
  const OVERSHOOT = 0.6;
  const VB_MIN = -OVERSHOOT;
  const VB_SPAN = 1 + OVERSHOOT * 2;

  let svgEl: SVGSVGElement | null = $state(null);
  let dragging: "p1" | "p2" | null = $state(null);
  let activePointerId = $state<number | null>(null);

  const curvePath = $derived.by(() => {
    const pts = sampleCurve(value, 48);
    return pts
      .map(
        ([x, y], i) =>
          `${i === 0 ? "M" : "L"} ${x.toFixed(4)} ${(1 - y).toFixed(4)}`,
      )
      .join(" ");
  });

  const selectedPresetId = $derived(
    EASING_PRESETS.find((p) => easingEquals(p.value, value))?.id ?? null,
  );

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
    const nx = Math.max(0, Math.min(1, x));
    // Let y stay within the viewBox so overshoot is reachable by drag.
    const ny = Math.max(VB_MIN, Math.min(1 + OVERSHOOT, y));
    if (which === "p1") {
      onchange({ ...value, x1: nx, y1: ny });
    } else {
      onchange({ ...value, x2: nx, y2: ny });
    }
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

  function setField(field: "x1" | "y1" | "x2" | "y2", raw: string) {
    const n = Number(raw);
    if (Number.isNaN(n)) return;
    const clamped =
      field === "x1" || field === "x2" ? Math.max(0, Math.min(1, n)) : n;
    onchange({ ...value, [field]: clamped });
  }

  function applyPreset(preset: Easing) {
    onchange({ ...preset });
  }

  function numField(v: number): string {
    return v.toFixed(2);
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
    <!--
      The SVG surface receives pointermove/up to continue a drag started on a
      handle (standard drag UX). It has no role of its own — the semantics
      live on the two handle circles below, which expose slider roles so AT
      users can identify and value-read each control point.
    -->
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
        class="text-primary"
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

      <!-- P1 handle -->
      <circle
        cx={value.x1}
        cy={1 - value.y1}
        r="0.032"
        fill="currentColor"
        role="slider"
        tabindex="0"
        aria-label="Control point 1"
        aria-valuemin={0}
        aria-valuemax={1}
        aria-valuenow={value.x1}
        aria-valuetext="x {value.x1.toFixed(2)}, y {value.y1.toFixed(2)}"
        class={cn(
          "text-primary focus:outline-none",
          !disabled && "cursor-grab",
        )}
        style:cursor={dragging === "p1" ? "grabbing" : undefined}
        onpointerdown={(e) => handleStart("p1", e)}
      />

      <!-- P2 handle -->
      <circle
        cx={value.x2}
        cy={1 - value.y2}
        r="0.032"
        fill="currentColor"
        role="slider"
        tabindex="0"
        aria-label="Control point 2"
        aria-valuemin={0}
        aria-valuemax={1}
        aria-valuenow={value.x2}
        aria-valuetext="x {value.x2.toFixed(2)}, y {value.y2.toFixed(2)}"
        class={cn(
          "text-primary focus:outline-none",
          !disabled && "cursor-grab",
        )}
        style:cursor={dragging === "p2" ? "grabbing" : undefined}
        onpointerdown={(e) => handleStart("p2", e)}
      />
    </svg>
  </div>

  <!-- Numeric inputs -->
  <div class="grid grid-cols-4 gap-1.5">
    {#each [["x1", value.x1], ["y1", value.y1], ["x2", value.x2], ["y2", value.y2]] as const as [field, v] (field)}
      <label class="flex flex-col gap-0.5">
        <span
          class="text-[9px] font-mono uppercase tracking-wide text-muted-foreground"
          >{field}</span
        >
        <Input
          type="number"
          pattern="^-?(?:\d+|\d*\.\d+)$"
          step="0.01"
          {disabled}
          value={numField(v)}
          onchange={(e) =>
            setField(field, (e.currentTarget as HTMLInputElement).value)}
          class="h-6 rounded-sm px-1.5 text-[11px] font-mono tabular-nums text-foreground no-webkit"
        />
      </label>
    {/each}
  </div>

  {#if showPresets}
    <div class="flex flex-wrap gap-1">
      {#each EASING_PRESETS as preset (preset.id)}
        {@const isActive = selectedPresetId === preset.id}
        <button
          type="button"
          {disabled}
          onclick={() => applyPreset(preset.value)}
          class={cn(
            "h-6 rounded-sm border px-2 text-[10px] font-medium transition-colors",
            "focus:outline-none focus:ring-1 focus:ring-ring",
            isActive
              ? "border-primary bg-primary/10 text-primary"
              : "border-border bg-background text-muted-foreground hover:text-foreground",
          )}
        >
          {preset.label}
        </button>
      {/each}
    </div>
  {/if}
</div>
