<script lang="ts">
  import { cn } from "@recast/ui/utils";
  import type { Snippet } from "svelte";

  interface Props {
    label: string;
    value: number;
    min?: number;
    max?: number;
    step?: number;
    description?: string;
    icon?: Snippet;
    unit?: string;
    disabled?: boolean;
    onstart?: () => void;
    onchange?: (value: number) => void;
    oncommit?: (value: number) => void;
    formatValue?: (value: number, unit: string) => string;
  }

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
    onstart,
    onchange,
    oncommit,
    formatValue,
  }: Props = $props();

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
    "group/slider relative flex h-10 w-full select-none items-center overflow-hidden rounded-sm border border-border/40 bg-card/55 px-1.5 shadow-(--shadow-craft-inset) outline-none transition-colors duration-150",
    "focus-visible:ring-2 focus-visible:ring-primary/30",
    disabled
      ? "cursor-not-allowed opacity-50"
      : "cursor-ew-resize hover:border-border/60 hover:bg-card/70",
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
  <div
    class="pointer-events-none absolute inset-y-0.75 left-0.75 rounded-sm bg-foreground/[0.07] shadow-[0_4px_10px_rgba(0,0,0,0.18)]"
    style:width={clampedPercentage > 0
      ? `max(calc(${clampedPercentage}% - 6px), 0.5rem)`
      : "0px"}
  ></div>

  <div
    class={cn(
      "pointer-events-none absolute inset-y-[18%] z-10 w-0.5 rounded-full bg-primary/95 shadow-[0_0_10px_var(--primary)]/30 transition-transform duration-150",
      "group-hover/slider:scale-y-105",
      isDragging && "scale-y-110",
    )}
    style:left={`calc(${clampedPercentage}% - 8px)`}
  ></div>

  <div class="pointer-events-none relative z-10 flex min-w-0 flex-1 items-center gap-1.5 pl-2.5">
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
    class="pointer-events-none relative z-10 shrink-0 pr-2.5 font-mono text-[12px] font-medium tabular-nums text-primary"
  >
    {formattedValue}
  </span>
</div>
