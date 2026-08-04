<script lang="ts">
  import { EASING_PRESETS, easingEquals } from "$lib/easing/cubic-bezier";
  import {
    defaultSpec,
    intensityRange,
    MAX_ANIM_MS,
    MIN_ANIM_MS,
    type SceneAnimKind,
    type SceneAnimDir,
    type SceneAnimSpec,
  } from "$lib/scenes/segment-anim";
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import { ArrowDown, ArrowLeft, ArrowRight, ArrowUp, Clock, Move3d } from "@recast/icons";
  import type { IconComponent } from "@recast/icons";
  import { Button } from "@recast/ui/button";
  import { SliderControl } from "@recast/ui/slider-control";
  import { cn } from "@recast/ui/utils";

  // One side (entrance or exit) of a segment's scene animation. Reads/writes the
  // spec through `store.setSegmentAnim` (coalesced undo). Kept dumb: all state is
  // the store's; this only maps controls onto a SceneAnimSpec.

  interface Props {
    store: EditorStore;
    /** Original start anchor of the selected segment. */
    start: number;
    side: "in" | "out";
  }
  let { store, start, side }: Props = $props();

  const KINDS: { id: SceneAnimKind; label: string }[] = [
    { id: "fade", label: "Fade" },
    { id: "slide", label: "Slide" },
    { id: "scale", label: "Scale" },
    { id: "shrink", label: "Shrink" },
    { id: "pop", label: "Pop" },
    { id: "rotate", label: "Rotate" },
  ];
  const DIRS: { id: SceneAnimDir; icon: IconComponent }[] = [
    { id: "left", icon: ArrowLeft },
    { id: "right", icon: ArrowRight },
    { id: "up", icon: ArrowUp },
    { id: "down", icon: ArrowDown },
  ];

  const spec = $derived<SceneAnimSpec | null>(
    store.segmentAnimAt(start)?.[side] ?? null,
  );
  const range = $derived(spec ? intensityRange(spec.kind) : null);
  const intensityValue = $derived(spec?.intensity ?? range?.default ?? 0);

  function write(next: SceneAnimSpec | null) {
    store.setSegmentAnim(start, side, next);
  }
  function pickKind(kind: SceneAnimKind) {
    const base = defaultSpec(kind, side, store.motionTone);
    // Keep the user's tuning (duration/easing) when swapping the kind.
    write({ ...base, durationMs: spec?.durationMs ?? base.durationMs, easing: spec?.easing ?? base.easing });
  }
  function patch(part: Partial<SceneAnimSpec>) {
    if (spec) write({ ...spec, ...part });
  }
</script>

<div class="space-y-2">
  <div class="grid grid-cols-4 gap-1">
    <button
      type="button"
      onclick={() => write(null)}
      aria-pressed={spec === null}
      class={cn(
        "rounded-md border px-1.5 py-1 text-[11px] font-medium transition-colors",
        spec === null
          ? "border-primary/60 bg-primary/10 text-primary"
          : "border-border/60 bg-card/40 text-muted-foreground hover:border-border hover:text-foreground",
      )}
    >
      Off
    </button>
    {#each KINDS as k (k.id)}
      {@const active = spec?.kind === k.id}
      <button
        type="button"
        onclick={() => pickKind(k.id)}
        aria-pressed={active}
        class={cn(
          "rounded-md border px-1.5 py-1 text-[11px] font-medium transition-colors",
          active
            ? "border-primary/60 bg-primary/10 text-primary"
            : "border-border/60 bg-card/40 text-muted-foreground hover:border-border hover:text-foreground",
        )}
      >
        {k.label}
      </button>
    {/each}
  </div>

  {#if spec}
    {#if spec.kind === "slide"}
      <div class="flex items-center gap-1">
        <span class="mr-1 text-[10px] text-muted-foreground">From</span>
        {#each DIRS as d (d.id)}
          {@const active = (spec.dir ?? "left") === d.id}
          <Button
            type="button"
            size="xs"
            aria-pressed={active}
            variant={active ? "default_soft" : "outline"}
            onclick={() => patch({ dir: d.id })}
          >
            <d.icon size={12} />
          </Button>
        {/each}
      </div>
    {/if}

    {#if range}
      <SliderControl
        label={range.label}
        value={intensityValue}
        min={range.min}
        max={range.max}
        step={range.step}
        unit={range.unit}
        formatValue={(v) =>
          range.unit === "°" ? `${Math.round(v)}°` : `${v.toFixed(2)}${range.unit}`}
        onchange={(v) => patch({ intensity: v })}
      >
        {#snippet icon()}
          <Move3d class="size-3" />
        {/snippet}
      </SliderControl>
    {/if}

    <SliderControl
      label="Duration"
      value={spec.durationMs}
      min={MIN_ANIM_MS}
      max={MAX_ANIM_MS}
      step={50}
      unit="ms"
      formatValue={(v) => `${Math.round(v)}ms`}
      onchange={(v) => patch({ durationMs: v })}
    >
      {#snippet icon()}
        <Clock class="size-3" />
      {/snippet}
    </SliderControl>

    <div class="flex flex-wrap gap-1">
      {#each EASING_PRESETS as preset (preset.id)}
        {@const active = easingEquals(spec.easing, preset.value)}
        <Button
          type="button"
          size="xs"
          aria-pressed={active}
          variant={active ? "default_soft" : "outline"}
          onclick={() => patch({ easing: preset.value })}
        >
          {preset.label}
        </Button>
      {/each}
    </div>
  {/if}
</div>
