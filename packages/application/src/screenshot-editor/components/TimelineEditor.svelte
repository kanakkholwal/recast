<script lang="ts" module>
import type { ScreenshotEditorState } from "../editor.svelte";

export interface TimelineEditorProps {
	editor: ScreenshotEditorState;
	onclose: () => void;
}

/** Track geometry (mirrors the clone's timeline constants). */
const PIXELS_PER_SECOND = 105;
const LABEL_WIDTH = 120;
const MIN_CLIP_MS = 200;
</script>

<script lang="ts">
  import { Button } from "@recast/ui/button";
  import { cn } from "@recast/ui/utils";
  import { Film, GitCommit, Pause, Play, Plus, Repeat, Trash2, X } from "@recast/icons";

  let { editor, onclose }: TimelineEditorProps = $props();

  let trackEl = $state<HTMLElement | null>(null);
  // What the current pointer drag is manipulating.
  type Drag =
    | { kind: "scrub" }
    | { kind: "move"; grabOffsetMs: number }
    | { kind: "resize" }
    | { kind: "keyframe"; id: string };
  let drag = $state<Drag | null>(null);

  const pxPerMs = PIXELS_PER_SECOND / 1000;
  const contentWidth = $derived(editor.timelineDuration * pxPerMs);
  const seconds = (ms: number) => (ms / 1000).toFixed(1);
  // One tick per second across the track.
  const ticks = $derived(
    Array.from({ length: Math.floor(editor.timelineDuration / 1000) + 1 }, (_, i) => i),
  );

  function timeAt(clientX: number): number {
    if (!trackEl) return 0;
    const rect = trackEl.getBoundingClientRect();
    const x = clientX - rect.left + trackEl.scrollLeft;
    return Math.max(0, Math.min(editor.timelineDuration, x / pxPerMs));
  }

  function startScrub(e: PointerEvent) {
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag = { kind: "scrub" };
    editor.seek(timeAt(e.clientX));
  }

  function startMove(e: PointerEvent) {
    e.stopPropagation();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag = { kind: "move", grabOffsetMs: timeAt(e.clientX) - editor.clipStart };
  }

  function startResize(e: PointerEvent) {
    e.stopPropagation();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag = { kind: "resize" };
  }

  function startKeyframeDrag(e: PointerEvent, id: string) {
    e.stopPropagation();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    editor.selectKeyframe(id);
    drag = { kind: "keyframe", id };
  }

  function onPointerMove(e: PointerEvent) {
    if (!drag) return;
    const t = timeAt(e.clientX);
    if (drag.kind === "scrub") {
      editor.seek(t);
    } else if (drag.kind === "move") {
      editor.setClip(t - drag.grabOffsetMs, editor.clipLength);
    } else if (drag.kind === "keyframe") {
      editor.moveKeyframe(drag.id, t);
    } else {
      editor.setClip(editor.clipStart, Math.max(MIN_CLIP_MS, t - editor.clipStart));
    }
  }

  function endDrag() {
    drag = null;
  }

  const SEEK_STEP = 100; // ms per arrow press
  const NUDGE = 100; // ms per arrow press when moving/resizing the clip

  function onRulerKey(e: KeyboardEvent) {
    if (e.key === "ArrowLeft") {
      e.preventDefault();
      editor.seek(editor.playhead - SEEK_STEP);
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      editor.seek(editor.playhead + SEEK_STEP);
    } else if (e.key === "Home") {
      e.preventDefault();
      editor.seek(0);
    } else if (e.key === "End") {
      e.preventDefault();
      editor.seek(editor.timelineDuration);
    }
  }

  function onClipKey(e: KeyboardEvent) {
    if (e.key === "ArrowLeft") {
      e.preventDefault();
      editor.setClip(editor.clipStart - NUDGE, editor.clipLength);
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      editor.setClip(editor.clipStart + NUDGE, editor.clipLength);
    }
  }

  function onResizeKey(e: KeyboardEvent) {
    if (e.key === "ArrowLeft") {
      e.preventDefault();
      editor.setClip(editor.clipStart, editor.clipLength - NUDGE);
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      editor.setClip(editor.clipStart, editor.clipLength + NUDGE);
    }
  }

  function onKeyframeKey(e: KeyboardEvent, id: string, time: number) {
    if (e.key === "ArrowLeft") {
      e.preventDefault();
      editor.moveKeyframe(id, time - NUDGE);
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      editor.moveKeyframe(id, time + NUDGE);
    } else if (e.key === "Delete" || e.key === "Backspace") {
      e.preventDefault();
      editor.removeKeyframe(id);
    } else if (e.key === "Enter") {
      e.preventDefault();
      editor.selectKeyframe(id);
    }
  }
</script>

<!-- Bottom timeline. Height mirrors the clone's 210px track editor. -->
<div class="border-border bg-card flex h-[210px] shrink-0 flex-col border-t">
  <!-- Controls -->
  <div class="border-border/40 flex h-11 shrink-0 items-center gap-2 border-b px-3">
    <Button
      variant="default"
      size="icon"
      class="size-7"
      aria-label={editor.playing ? "Pause" : "Play"}
      disabled={!editor.animationId && !editor.keyframeMode}
      onclick={() => editor.togglePlay()}
    >
      {#if editor.playing}<Pause class="size-3.5" />{:else}<Play class="size-3.5" />{/if}
    </Button>

    <Button
      variant="secondary"
      size="sm"
      class="h-7 gap-1"
      title="Capture the current 3D look as a keyframe at the playhead"
      onclick={() => editor.addKeyframe()}
    >
      <Plus class="size-3.5" />
      Key
    </Button>

    <span class="text-muted-foreground w-24 font-mono text-xs tabular-nums">
      {seconds(editor.playhead)}s / {seconds(editor.timelineDuration)}s
    </span>

    <Button
      variant={editor.loop ? "secondary" : "ghost"}
      size="icon"
      class="size-7"
      aria-label="Loop playback"
      aria-pressed={editor.loop}
      title="Loop"
      onclick={() => editor.toggleLoop()}
    >
      <Repeat class="size-3.5" />
    </Button>

    <div class="ml-2 flex items-center gap-2">
      <label class="text-muted-foreground text-xs" for="timeline-duration">Duration</label>
      <input
        id="timeline-duration"
        type="range"
        class="accent-primary h-1.5 w-28 cursor-pointer"
        min="1"
        max="30"
        step="1"
        value={Math.round(editor.timelineDuration / 1000)}
        oninput={(e) => editor.setTimelineDuration(Number(e.currentTarget.value) * 1000)}
      />
      <span class="text-muted-foreground w-8 font-mono text-xs tabular-nums">
        {Math.round(editor.timelineDuration / 1000)}s
      </span>
    </div>

    <div class="flex-1"></div>

    {#if editor.keyframeMode}
      <Button variant="ghost" size="sm" onclick={() => editor.clearKeyframes()}>
        <Trash2 />
        Clear keys
      </Button>
    {:else if editor.animationId}
      <Button variant="ghost" size="sm" onclick={() => editor.clearAnimation()}>
        <Trash2 />
        Clear
      </Button>
    {/if}
    <Button variant="ghost" size="icon" class="size-7" aria-label="Close timeline" onclick={onclose}>
      <X class="size-3.5" />
    </Button>
  </div>

  <!-- Tracks -->
  <div class="flex min-h-0 flex-1">
    <!-- Labels -->
    <div
      class="border-border/40 flex shrink-0 flex-col border-r"
      style:width={`${LABEL_WIDTH}px`}
    >
      <div class="border-border/30 h-6 border-b"></div>
      <div class="flex h-12 items-center gap-1.5 px-3">
        {#if editor.keyframeMode}
          <GitCommit class="text-muted-foreground size-3.5" />
          <span class="text-muted-foreground text-xs font-medium">Keyframes</span>
        {:else}
          <Film class="text-muted-foreground size-3.5" />
          <span class="text-muted-foreground text-xs font-medium">Animation</span>
        {/if}
      </div>
    </div>

    <!-- Scrollable track area -->
    <div bind:this={trackEl} class="relative min-w-0 flex-1 overflow-x-auto">
      <div class="relative" style:width={`${contentWidth}px`} style:min-width="100%">
        <!-- Time ruler: click/drag to scrub, arrows to step. -->
        <div
          class="border-border/30 focus-visible:ring-primary/40 relative h-6 cursor-ew-resize border-b outline-none select-none focus-visible:ring-2"
          role="slider"
          tabindex="0"
          aria-label="Playhead"
          aria-valuemin={0}
          aria-valuemax={editor.timelineDuration}
          aria-valuenow={Math.round(editor.playhead)}
          aria-valuetext={`${seconds(editor.playhead)} seconds`}
          onkeydown={onRulerKey}
          onpointerdown={startScrub}
          onpointermove={onPointerMove}
          onpointerup={endDrag}
          onpointercancel={endDrag}
        >
          {#each ticks as t (t)}
            <span
              class="bg-border/70 absolute top-0 h-2 w-px"
              style:left={`${t * 1000 * pxPerMs}px`}
            ></span>
            <span
              class="text-muted-foreground absolute top-2 font-mono text-[9px] tabular-nums"
              style:left={`${t * 1000 * pxPerMs + 3}px`}
            >
              {t}s
            </span>
          {/each}
        </div>

        <!-- Animation track -->
        <div class="bg-muted/20 relative h-12">
          {#if editor.keyframeMode}
            <!-- Keyframe diamonds: drag to retime, click to edit, Delete to remove. -->
            {#each editor.keyframes as kf (kf.id)}
              <button
                type="button"
                class={cn(
                  "absolute top-1/2 size-3 -translate-x-1/2 -translate-y-1/2 rotate-45 cursor-grab border outline-none focus-visible:ring-2",
                  editor.selectedKeyframeId === kf.id
                    ? "border-primary bg-primary ring-primary/40"
                    : "border-primary/70 bg-background hover:bg-primary/30 focus-visible:ring-primary/40",
                )}
                style:left={`${kf.time * pxPerMs}px`}
                aria-label={`Keyframe at ${seconds(kf.time)}s. Arrow keys retime, Delete removes.`}
                onpointerdown={(e) => startKeyframeDrag(e, kf.id)}
                onpointermove={onPointerMove}
                onpointerup={endDrag}
                onpointercancel={endDrag}
                onkeydown={(e) => onKeyframeKey(e, kf.id, kf.time)}
                onclick={() => editor.selectKeyframe(kf.id)}
              ></button>
            {/each}
            {#if editor.keyframes.length < 2}
              <p class="text-muted-foreground pointer-events-none px-3 py-3.5 text-xs">
                Set a 3D look, then press <span class="text-foreground font-medium">Key</span> to add another keyframe.
              </p>
            {/if}
          {:else if editor.animationPreset}
            <!-- Clip body and its resize handle are siblings (not nested), so
                 both stay keyboard-reachable and validly interactive. -->
            <div
              class="absolute inset-y-1.5"
              style:left={`${editor.clipStart * pxPerMs}px`}
              style:width={`${editor.clipLength * pxPerMs}px`}
            >
              <button
                type="button"
                class={cn(
                  "bg-primary/85 text-primary-foreground focus-visible:ring-primary/50 absolute inset-0 flex cursor-grab items-center overflow-hidden rounded-md outline-none focus-visible:ring-2",
                  drag?.kind === "move" && "cursor-grabbing",
                )}
                aria-label={`${editor.animationPreset.name} clip: ${seconds(editor.clipStart)}s to ${seconds(editor.clipEnd)}s. Arrow keys move it.`}
                onkeydown={onClipKey}
                onpointerdown={startMove}
                onpointermove={onPointerMove}
                onpointerup={endDrag}
                onpointercancel={endDrag}
              >
                <span class="truncate px-2 text-[11px] font-medium">
                  {editor.animationPreset.name}
                </span>
              </button>
              <button
                type="button"
                class="hover:bg-primary-foreground/30 focus-visible:ring-primary-foreground/60 absolute inset-y-0 right-0 w-2 cursor-ew-resize rounded-r-md outline-none focus-visible:ring-2"
                aria-label={`Resize clip. Length ${seconds(editor.clipLength)} seconds. Arrow keys adjust.`}
                onkeydown={onResizeKey}
                onpointerdown={startResize}
                onpointermove={onPointerMove}
                onpointerup={endDrag}
                onpointercancel={endDrag}
              ></button>
            </div>
          {:else}
            <p class="text-muted-foreground px-3 py-3.5 text-xs">
              Pick a motion in the Motion tab to add a clip.
            </p>
          {/if}
        </div>

        <!-- Playhead spans both tracks -->
        <div
          class="bg-primary pointer-events-none absolute top-0 bottom-0 z-10 w-px"
          style:left={`${editor.playhead * pxPerMs}px`}
        >
          <span
            class="bg-primary absolute -top-0 -left-1 size-2 rounded-full"
            aria-hidden="true"
          ></span>
        </div>
      </div>
    </div>
  </div>
</div>
