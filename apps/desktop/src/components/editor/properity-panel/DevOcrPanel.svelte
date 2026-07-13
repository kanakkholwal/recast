<script lang="ts">
  // Dev-only review surface for the experimental on-device OCR pass. Reads the
  // recording into screen-state spans and lists them with a frame preview so the
  // output can be eyeballed against the real video before any of this is wired
  // into the agent/CLI surface for real. Never rendered in a production build.
  import { clock } from "$lib/format/time";
  import { readVideoText, type VideoTextTimeline } from "$lib/ipc";
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import { Button } from "@recast/ui/button";
  import { FlaskConical, Loader2, ScanText } from "@lucide/svelte";
  import PanelSection from "./PanelSection.svelte";

  interface Props {
    store: EditorStore;
  }
  let { store }: Props = $props();

  let status = $state<"idle" | "running" | "ready" | "error">("idle");
  let phase = $state("");
  let error = $state<string | null>(null);
  let timeline = $state<VideoTextTimeline | null>(null);
  let elapsedMs = $state(0);

  // The raw .mp4 on disk. `store.videoPath` is the .recast container, which the
  // OCR command cannot decode directly. This file is the screen capture ONLY:
  // camera, background, zoom and annotations are composited at export, never
  // baked into the source, so OCR sees just the recorded screen.
  const mediaPath = $derived(store.recordingPath);

  // The footage the edit actually keeps, in original-recording seconds. Passing
  // this means trimmed-off and cut-out regions are never read, so we don't produce
  // spans for content that isn't in the video (and don't pay OCR for it).
  const keptRanges = $derived(
    store.segments.map((s) => [s.start, s.end] as [number, number]),
  );

  const phaseLabel = $derived(
    phase === "downloading"
      ? "Fetching OCR models (first run only)"
      : phase === "reading"
        ? "Sampling frames and reading text"
        : "Working",
  );

  async function run() {
    if (!mediaPath) return;
    status = "running";
    error = null;
    timeline = null;
    phase = "";
    const started = performance.now();
    try {
      timeline = await readVideoText({
        videoPath: mediaPath,
        previews: true,
        includeRanges: keptRanges,
        onPhase: (p) => {
          phase = p.phase;
        },
      });
      elapsedMs = Math.round(performance.now() - started);
      status = "ready";
    } catch (e) {
      error = `${e}`;
      status = "error";
    } finally {
      phase = "";
    }
  }
</script>

<div class="flex flex-col gap-3">
  <PanelSection title="Screen text (experimental)">
    <p class="text-muted-foreground text-xs leading-relaxed">
      Reads the screen recording into timestamped spans using on-device OCR. Only
      the footage you keep is read, so trims and cuts are skipped, and the camera
      is never included. Dev builds only; the first run downloads about 12 MB of
      models.
    </p>

    {#if !mediaPath}
      <p class="text-muted-foreground mt-3 text-xs">
        No source recording is loaded.
      </p>
    {:else}
      <Button
        class="mt-3 w-full"
        variant="secondary"
        size="sm"
        disabled={status === "running"}
        onclick={run}
      >
        {#if status === "running"}
          <Loader2 class="size-4 animate-spin" />
          Reading…
        {:else}
          <ScanText class="size-4" />
          {timeline ? "Read again" : "Read screen text"}
        {/if}
      </Button>
    {/if}

    {#if status === "running" && phase}
      <p class="text-muted-foreground mt-2 text-xs" aria-live="polite">
        {phaseLabel}…
      </p>
    {/if}

    {#if status === "error" && error}
      <p class="text-destructive mt-2 text-xs" role="alert">{error}</p>
    {/if}

    {#if status === "ready" && timeline}
      <p class="text-muted-foreground mt-2 text-xs tabular-nums" aria-live="polite">
        {timeline.spans.length}
        {timeline.spans.length === 1 ? "span" : "spans"} · {timeline.engine} · {(
          elapsedMs / 1000
        ).toFixed(1)}s
      </p>
    {/if}
  </PanelSection>

  {#if status === "ready" && timeline}
    {#if timeline.spans.length === 0}
      <p class="text-muted-foreground px-1 text-xs">
        No text was found in this recording.
      </p>
    {:else}
      <ul class="flex flex-col gap-2">
        {#each timeline.spans as span (span.start)}
          <li>
            <button
              type="button"
              class="border-border hover:border-primary/60 hover:bg-accent/40 focus-visible:ring-ring flex w-full flex-col gap-2 rounded-lg border p-2 text-left transition-colors focus-visible:ring-2 focus-visible:outline-none"
              onclick={() => store.seek(span.start)}
              title="Jump to {clock(span.start)}"
            >
              <div class="flex items-center justify-between gap-2">
                <span class="text-xs font-medium tabular-nums">
                  {clock(span.start)} – {clock(span.end)}
                </span>
                <span class="text-muted-foreground text-[10px] tabular-nums">
                  {span.elements.length}
                  {span.elements.length === 1 ? "line" : "lines"}
                </span>
              </div>

              {#if span.preview}
                <img
                  src={span.preview}
                  alt="Frame at {clock(span.start)}"
                  class="border-border w-full rounded border"
                  loading="lazy"
                />
              {/if}

              {#if span.elements.length > 0}
                <p class="text-muted-foreground line-clamp-3 text-[11px] leading-snug">
                  {span.elements.map((e) => e.content).join(" · ")}
                </p>
              {:else}
                <p class="text-muted-foreground text-[11px] italic">No text read</p>
              {/if}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  {/if}

  {#if status === "idle"}
    <p
      class="text-muted-foreground flex items-center gap-1.5 px-1 text-[11px] leading-relaxed"
    >
      <FlaskConical class="size-3.5 shrink-0" />
      Experimental. Accuracy on dense UI text is not yet validated.
    </p>
  {/if}
</div>
