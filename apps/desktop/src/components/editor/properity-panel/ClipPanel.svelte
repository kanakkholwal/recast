<script lang="ts">
  import { clockCentis } from "$lib/format/time";
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import {
    MAX_SEGMENT_SPEED,
    MIN_SEGMENT_SPEED,
  } from "$lib/timeline/segment-speed";
  import {
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    Gauge,
    RotateCcw,
    SquareSplitHorizontal,
    Trash2,
  } from "@recast/icons";
  import type { SeamTransition } from "$lib/scenes/seam";
  import { Button } from "@recast/ui/button";
  import { Kbd } from "@recast/ui/kbd";
  import { SliderControl } from "@recast/ui/slider-control";
  import { cn } from "@recast/ui/utils";
  import { anchorMatches, fmtSpeed } from "./clip-panel.logic";
  import PanelSection from "./PanelSection.svelte";
  import SceneAnimControls from "./SceneAnimControls.svelte";

  // Contextual controls for the clip/segment selected on the timeline. Auto-opened
  // by PropertiesPanel when `selectedClipStart` is set (mirrors the Focus tab for
  // zoom regions). Speed writes go through `store.setSegmentSpeed` (coalesced undo).

  interface Props {
    store: EditorStore;
  }
  let { store }: Props = $props();

  const SPEED_PRESETS = [0.5, 1, 1.5, 2];

  // Project-wide scene-animation motion style (restyles every clip's animation).
  const MOTION_TONES = [
    { id: "subtle", label: "Subtle" },
    { id: "balanced", label: "Balanced" },
    { id: "energetic", label: "Energetic" },
  ] as const;

  // The selected kept segment, matched by its original start anchor.
  const selected = $derived.by(() => {
    const start = store.selectedClipStart;
    if (start === null) return null;
    return store.segments.find((s) => anchorMatches(s.start, start)) ?? null;
  });
  const speed = $derived(selected ? store.segmentSpeedAt(selected.start) : 1);
  const isSped = $derived(!anchorMatches(speed, 1));

  // A seam sits before this clip when a cut removed content between it and the
  // previous segment. That's where a transition smooths the jump.
  const prevSeg = $derived(
    selected && selected.index > 0
      ? (store.segments.find((s) => s.index === selected.index - 1) ?? null)
      : null,
  );
  const seamBefore = $derived(!!prevSeg && !!selected && selected.start - prevSeg.end > 1e-4);
  const seamKind = $derived(
    seamBefore && prevSeg && selected ? store.seamTransitionAt(prevSeg.start, selected.start) : "none",
  );
  const SEAM_TRANSITIONS: { id: SeamTransition; label: string; icon?: typeof ArrowLeft }[] = [
    { id: "none", label: "None" },
    { id: "dip", label: "Dip" },
    { id: "push-left", label: "", icon: ArrowLeft },
    { id: "push-right", label: "", icon: ArrowRight },
    { id: "push-up", label: "", icon: ArrowUp },
    { id: "push-down", label: "", icon: ArrowDown },
  ];
  function setSeam(kind: SeamTransition) {
    if (prevSeg && selected) store.setSeamTransition(prevSeg.start, selected.start, kind);
  }

  function setSpeed(v: number) {
    if (selected) store.setSegmentSpeed(selected.start, v);
  }
  function splitHere() {
    store.splitAt(store.currentTime);
  }
  function deleteClip() {
    if (!selected) return;
    const joinAt = store.deleteSegmentAt((selected.start + selected.end) / 2);
    if (joinAt !== null) store.seek(joinAt);
  }
</script>

{#if !selected}
  <div
    class="flex flex-col items-center justify-center gap-2 px-3 py-12 text-center animate-in fade-in duration-200"
  >
    <SquareSplitHorizontal class="size-6 text-muted-foreground/50" />
    <p class="text-[11px] leading-snug text-muted-foreground">
      Select a clip on the timeline to change its speed, split it, or remove it.
    </p>
  </div>
{:else}
  {@const duration = selected.end - selected.start}
  <div class="space-y-3 animate-in fade-in duration-200">
    <div class="rounded-lg border border-border/60 bg-card/40 px-3 py-2">
      <div class="flex items-baseline justify-between">
        <span class="text-[11px] text-muted-foreground">Clip duration</span>
        <span class="font-mono text-[12px] tabular-nums text-foreground">
          {clockCentis(duration)}
        </span>
      </div>
      {#if isSped}
        <div class="mt-0.5 flex items-baseline justify-between text-[10px] text-muted-foreground">
          <span>Plays in</span>
          <span class="font-mono tabular-nums text-primary">
            {clockCentis(duration / speed)} at {fmtSpeed(speed)}
          </span>
        </div>
      {/if}
    </div>

    <PanelSection
      title="Clip speed"
      hint="How fast this clip plays, in both preview and export. 1× is normal."
    >
      {#snippet action()}
        {#if isSped}
          <button
            type="button"
            onclick={() => setSpeed(1)}
            class="flex items-center gap-1 text-[10px] text-muted-foreground transition-colors hover:text-foreground"
          >
            <RotateCcw class="size-2.5" />
            Reset
          </button>
        {/if}
      {/snippet}
      <div class="grid grid-cols-4 gap-1">
        {#each SPEED_PRESETS as preset (preset)}
          {@const active = anchorMatches(speed, preset)}
          <button
            type="button"
            onclick={() => setSpeed(preset)}
            aria-pressed={active}
            class={cn(
              "rounded-md border px-1.5 py-1 font-mono text-[11px] font-semibold tabular-nums transition-colors",
              active
                ? "border-primary/60 bg-primary/10 text-primary"
                : "border-border/60 bg-card/40 text-muted-foreground hover:border-border hover:text-foreground",
            )}
          >
            {fmtSpeed(preset)}
          </button>
        {/each}
      </div>
      <SliderControl
        label="Fine"
        value={speed}
        min={MIN_SEGMENT_SPEED}
        max={MAX_SEGMENT_SPEED}
        step={0.05}
        unit="×"
        formatValue={(v) => `${v.toFixed(2)}×`}
        onchange={(v) => setSpeed(v)}
      >
        {#snippet icon()}
          <Gauge class="size-3" />
        {/snippet}
      </SliderControl>
    </PanelSection>

    {#if seamBefore}
      <PanelSection
        title="Cut transition"
        hint="Smooth the jump where a cut removed content: this clip pushes in as the previous one pushes out."
      >
        <div class="grid grid-cols-5 gap-1">
          {#each SEAM_TRANSITIONS as t (t.id)}
            {@const active = seamKind === t.id}
            {@const seamLabel =
              t.id === "none"
                ? "No transition"
                : t.id === "dip"
                  ? "Dip to background"
                  : t.id.replace("push-", "Push ")}
            <button
              type="button"
              onclick={() => setSeam(t.id)}
              aria-pressed={active}
              aria-label={seamLabel}
              title={seamLabel}
              class={cn(
                "flex items-center justify-center rounded-md border px-1.5 py-1 text-[11px] font-medium transition-colors",
                active
                  ? "border-primary/60 bg-primary/10 text-primary"
                  : "border-border/60 bg-card/40 text-muted-foreground hover:border-border hover:text-foreground",
              )}
            >
              {#if t.icon}
                <t.icon class="size-3.5" />
              {:else}
                {t.label}
              {/if}
            </button>
          {/each}
        </div>
        {#if seamKind === "custom"}
          <p class="mt-1 text-[10px] text-muted-foreground/70">
            Custom entrance/exit set on these clips. Pick a push to replace it.
          </p>
        {/if}
      </PanelSection>
    {/if}

    <PanelSection
      title="Scene animation"
      hint="How this clip animates into and out of view. Applies to the video layer only, in both preview and export."
    >
      <div class="space-y-3">
        <div class="space-y-1.5">
          <div class="flex items-center justify-between">
            <span class="text-[10px] font-medium uppercase tracking-wide text-muted-foreground">Motion</span>
            <span class="text-[10px] text-muted-foreground/70">all clips</span>
          </div>
          <div class="grid grid-cols-3 gap-1">
            {#each MOTION_TONES as tone (tone.id)}
              {@const active = store.motionTone === tone.id}
              <button
                type="button"
                onclick={() => store.setMotionTone(tone.id)}
                aria-pressed={active}
                class={cn(
                  "rounded-md border px-1.5 py-1 text-[11px] font-medium transition-colors",
                  active
                    ? "border-primary/60 bg-primary/10 text-primary"
                    : "border-border/60 bg-card/40 text-muted-foreground hover:border-border hover:text-foreground",
                )}
              >
                {tone.label}
              </button>
            {/each}
          </div>
        </div>
        <div class="space-y-1.5">
          <span class="text-[10px] font-medium uppercase tracking-wide text-muted-foreground">Entrance</span>
          <SceneAnimControls {store} start={selected.start} side="in" />
        </div>
        <div class="space-y-1.5">
          <span class="text-[10px] font-medium uppercase tracking-wide text-muted-foreground">Exit</span>
          <SceneAnimControls {store} start={selected.start} side="out" />
        </div>
      </div>
    </PanelSection>

    <div class="space-y-1.5">
      <Button
        variant="outline"
        size="sm"
        class="w-full justify-start gap-2"
        onclick={splitHere}
      >
        <SquareSplitHorizontal class="size-3.5" />
        Split at playhead
        <Kbd class="ml-auto">S</Kbd>
      </Button>
      {#if store.segments.length > 1}
        <Button
          variant="outline"
          size="sm"
          class="w-full justify-start gap-2 text-destructive hover:text-destructive"
          onclick={deleteClip}
        >
          <Trash2 class="size-3.5" />
          Delete clip
        </Button>
      {/if}
    </div>
  </div>
{/if}
