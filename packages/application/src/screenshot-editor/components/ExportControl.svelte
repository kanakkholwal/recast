<script lang="ts" module>
import type { ScreenshotEditorState } from "../editor.svelte";

export interface ExportControlProps {
	editor: ScreenshotEditorState;
	/** Returns the live stage node to snapshot (null before mount). */
	getStage: () => HTMLElement | null;
	/** App hook for toasts; the package stays decoupled from any toaster. */
	onnotify?: (message: string, kind: "success" | "error") => void;
}
</script>

<script lang="ts">
  import { tick } from "svelte";
  import { PanelSection } from "@recast/ui/panel-section";
  import { Segmented } from "@recast/ui/segmented";
  import { SliderControl } from "@recast/ui/slider-control";
  import { Button } from "@recast/ui/button";
  import { Copy, Download as DownloadIcon, Film, ImagePlus, Loader2, Package, X } from "@recast/icons";
  import {
    canCopyImage,
    copyToClipboard,
    defaultFilename,
    download,
    snapshot,
  } from "../export";
  import { canExportVideo, canExportWebM, canExportAnyVideo, exportVideo, exportVideoWebM } from "../video";
  import { imageFromFile } from "../image-input";
  import { zipStore } from "../zip";
  import type { ExportFormat } from "../types";

  let { editor, getStage, onnotify }: ExportControlProps = $props();

  let busy = $state<null | "download" | "copy" | "batch">(null);
  let batchProgress = $state(0);
  let slidesInput = $state<HTMLInputElement | null>(null);
  const copyable = canCopyImage();

  const ext = (f: ExportFormat) => (f === "jpeg" ? "jpg" : f);

  async function onAddSlides(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const files = Array.from(input.files ?? []);
    input.value = "";
    for (const file of files) {
      try {
        editor.addSlide(await imageFromFile(file));
      } catch (err) {
        onnotify?.(err instanceof Error ? err.message : "Could not add slide", "error");
      }
    }
  }

  async function doBatch() {
    const node = getStage();
    if (!node || busy) return;
    busy = "batch";
    batchProgress = 0;
    const prev = editor.activeSlide;
    const base = editor.image?.name || "screenshot";
    try {
      const entries = [];
      let failed = 0;
      for (let i = 0; i < editor.slides.length; i++) {
        // Per-slide try/catch: one bad slide can't abort the whole zip.
        try {
          editor.setActiveSlide(i);
          await tick();
          // Ensure the slide's image is decoded before snapshotting the stage.
          const img = new Image();
          img.src = editor.slides[i].src;
          await img.decode().catch(() => {});
          await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
          const blob = await snapshot(node, {
            format: editor.exportFormat,
            scale: editor.exportScale,
            quality: editor.exportQuality,
          });
          // Index keeps names unique even if two slides share a filename.
          const name = `${editor.slides[i].name || base}-${i + 1}`;
          entries.push({
            name: `${name}.${ext(editor.exportFormat)}`,
            data: new Uint8Array(await blob.arrayBuffer()),
          });
        } catch {
          failed++;
        }
        batchProgress = (i + 1) / editor.slides.length;
      }
      if (entries.length === 0) throw new Error("no slides could be exported");
      download(zipStore(entries), `${base}-batch.zip`);
      onnotify?.(
        failed ? `Exported ${entries.length}, ${failed} failed` : `Exported ${entries.length} images`,
        failed ? "error" : "success",
      );
    } catch (e) {
      onnotify?.(e instanceof Error ? e.message : "Batch export failed", "error");
    } finally {
      editor.setActiveSlide(prev);
      busy = null;
    }
  }

  const videoSupported = canExportAnyVideo();
  const canMp4 = canExportVideo();
  const canWebm = canExportWebM();
  let videoFormat = $state<"mp4" | "webm">(canExportVideo() ? "mp4" : "webm");
  let videoBusy = $state(false);
  let videoProgress = $state(0);

  async function doVideo() {
    const node = getStage();
    const preset = editor.activePreset;
    if (!node || videoBusy) return;
    if (!preset) {
      onnotify?.("Pick a motion or add keyframes first", "error");
      return;
    }
    if (editor.playing) editor.togglePlay(); // freeze playback while capturing
    videoBusy = true;
    videoProgress = 0;
    try {
      const encode = videoFormat === "webm" ? exportVideoWebM : exportVideo;
      const blob = await encode(node, preset, 30, (p) => (videoProgress = p), {
        duration: editor.timelineDuration,
        clipStart: editor.clipStart,
        clipLength: editor.clipLength,
      });
      download(blob, `screenshot.${videoFormat}`);
      onnotify?.("Video saved", "success");
    } catch (e) {
      onnotify?.(e instanceof Error ? e.message : "Video export failed", "error");
    } finally {
      videoBusy = false;
    }
  }

  async function doDownload() {
    const node = getStage();
    if (!node || busy) return;
    busy = "download";
    try {
      const blob = await snapshot(node, {
        format: editor.exportFormat,
        scale: editor.exportScale,
        quality: editor.exportQuality,
      });
      download(blob, defaultFilename(editor.exportFormat, editor.image?.name || "screenshot"));
      onnotify?.("Image saved", "success");
    } catch (e) {
      onnotify?.(e instanceof Error ? e.message : "Export failed", "error");
    } finally {
      busy = null;
    }
  }

  async function doCopy() {
    const node = getStage();
    if (!node || busy) return;
    busy = "copy";
    try {
      await copyToClipboard(node, editor.exportScale);
      onnotify?.("Copied to clipboard", "success");
    } catch (e) {
      onnotify?.(e instanceof Error ? e.message : "Copy failed", "error");
    } finally {
      busy = null;
    }
  }
</script>

<PanelSection title="Export">
  <div class="flex flex-col gap-2.5">
    <Segmented
      options={[
        { value: "png", label: "PNG" },
        { value: "jpeg", label: "JPG" },
        { value: "webp", label: "WebP" },
      ]}
      value={editor.exportFormat}
      onValueChange={(v) => (editor.exportFormat = v as ExportFormat)}
      aria-label="Export format"
    />
    <Segmented
      options={[
        { value: "1", label: "1x" },
        { value: "2", label: "2x" },
        { value: "3", label: "3x" },
        { value: "4", label: "4x" },
      ]}
      value={String(editor.exportScale)}
      onValueChange={(v) => (editor.exportScale = Number(v))}
      aria-label="Export resolution"
    />
    {#if editor.exportFormat !== "png"}
      <SliderControl
        label="Quality"
        value={Math.round(editor.exportQuality * 100)}
        min={10}
        max={100}
        step={1}
        unit="%"
        onchange={(v) => (editor.exportQuality = v / 100)}
      />
    {/if}
    <div class="mt-1 flex gap-2">
      <Button class="flex-1" variant="default" size="sm" disabled={busy !== null} onclick={doDownload}>
        {#if busy === "download"}
          <Loader2 class="animate-spin" />
        {:else}
          <DownloadIcon />
        {/if}
        Download
      </Button>
      {#if copyable}
        <Button variant="outline" size="sm" disabled={busy !== null} onclick={doCopy} aria-label="Copy to clipboard">
          {#if busy === "copy"}
            <Loader2 class="animate-spin" />
          {:else}
            <Copy />
          {/if}
        </Button>
      {/if}
    </div>
    {#if editor.slides.length > 1}
      <Button variant="outline" size="sm" class="w-full" disabled={busy !== null} onclick={doBatch}>
        {#if busy === "batch"}
          <Loader2 class="animate-spin" />
          Zipping {Math.round(batchProgress * 100)}%
        {:else}
          <Package />
          Export all ({editor.slides.length})
        {/if}
      </Button>
    {/if}
  </div>
</PanelSection>

<!-- Slides: extra images that share the current design; "Export all" zips them. -->
{#if editor.hasImage}
  <input
    bind:this={slidesInput}
    type="file"
    accept="image/*"
    multiple
    class="hidden"
    onchange={onAddSlides}
  />
  <PanelSection title="Slides" collapsible defaultOpen={editor.slides.length > 1}>
    {#snippet action()}
      <Button variant="ghost" size="xs" onclick={() => slidesInput?.click()}>
        <ImagePlus />
        Add
      </Button>
    {/snippet}
    <div class="grid grid-cols-4 gap-2">
      {#each editor.slides as slide, i (slide.src + i)}
        <div class="relative">
          <button
            type="button"
            class="border-border aspect-video w-full overflow-hidden rounded-md border transition-transform hover:scale-105 {editor.activeSlide === i ? 'ring-primary ring-2 ring-offset-1' : ''}"
            aria-label={`Slide ${i + 1}`}
            aria-pressed={editor.activeSlide === i}
            onclick={() => editor.setActiveSlide(i)}
          >
            <img src={slide.src} alt="" class="size-full object-cover" />
          </button>
          {#if editor.slides.length > 1}
            <button
              type="button"
              class="bg-background/80 text-foreground hover:bg-destructive hover:text-destructive-foreground absolute -right-1 -top-1 rounded-full p-0.5 shadow"
              aria-label={`Remove slide ${i + 1}`}
              onclick={() => editor.removeSlide(i)}
            >
              <X class="size-3" />
            </button>
          {/if}
        </div>
      {/each}
    </div>
  </PanelSection>
{/if}

{#if videoSupported}
  <PanelSection title="Video">
    {#if editor.activePreset}
      <p class="text-muted-foreground text-xs">
        Motion: <span class="text-foreground font-medium">{editor.activePreset.name}</span>
        · {(editor.activePreset.duration / 1000).toFixed(1)}s
      </p>
      {#if canMp4 && canWebm}
        <Segmented
          options={[
            { value: "mp4", label: "MP4" },
            { value: "webm", label: "WebM" },
          ]}
          value={videoFormat}
          onValueChange={(v) => (videoFormat = v as "mp4" | "webm")}
          aria-label="Video format"
        />
      {/if}
      <Button class="w-full" variant="default" size="sm" disabled={videoBusy} onclick={doVideo}>
        {#if videoBusy}
          <Loader2 class="animate-spin" />
          Encoding {Math.round(videoProgress * 100)}%
        {:else}
          <Film />
          Export {videoFormat.toUpperCase()}
        {/if}
      </Button>
    {:else}
      <p class="text-muted-foreground text-xs">
        Choose a motion in the <span class="text-foreground font-medium">Animate</span> panel to export
        a clip.
      </p>
    {/if}
  </PanelSection>
{/if}
