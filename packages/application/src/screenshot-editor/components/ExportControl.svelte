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
  import { PanelSection } from "@recast/ui/panel-section";
  import { Segmented } from "@recast/ui/segmented";
  import { Button } from "@recast/ui/button";
  import { Copy, Download as DownloadIcon, Film, Loader2 } from "@lucide/svelte";
  import {
    canCopyImage,
    copyToClipboard,
    defaultFilename,
    download,
    snapshot,
  } from "../export";
  import { canExportVideo, exportVideo } from "../video";
  import type { ExportFormat } from "../types";

  let { editor, getStage, onnotify }: ExportControlProps = $props();

  let busy = $state<null | "download" | "copy">(null);
  const copyable = canCopyImage();

  const videoSupported = canExportVideo();
  let videoBusy = $state(false);
  let videoProgress = $state(0);

  async function doVideo() {
    const node = getStage();
    const preset = editor.animationPreset;
    if (!node || videoBusy) return;
    if (!preset) {
      onnotify?.("Pick a motion in the Animate panel first", "error");
      return;
    }
    if (editor.playing) editor.togglePlay(); // freeze playback while capturing
    videoBusy = true;
    videoProgress = 0;
    try {
      const blob = await exportVideo(node, preset, 30, (p) => (videoProgress = p));
      download(blob, "screenshot.mp4");
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
      });
      download(blob, defaultFilename(editor.exportFormat));
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
  </div>
</PanelSection>

{#if videoSupported}
  <PanelSection title="Video">
    {#if editor.animationPreset}
      <p class="text-muted-foreground text-xs">
        Motion: <span class="text-foreground font-medium">{editor.animationPreset.name}</span>
        · {(editor.animationPreset.duration / 1000).toFixed(1)}s
      </p>
      <Button class="w-full" variant="default" size="sm" disabled={videoBusy} onclick={doVideo}>
        {#if videoBusy}
          <Loader2 class="animate-spin" />
          Encoding {Math.round(videoProgress * 100)}%
        {:else}
          <Film />
          Export MP4
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
