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
  import { Copy, Download as DownloadIcon, Loader2 } from "@lucide/svelte";
  import {
    canCopyImage,
    copyToClipboard,
    defaultFilename,
    download,
    snapshot,
  } from "../export";
  import type { ExportFormat } from "../types";

  let { editor, getStage, onnotify }: ExportControlProps = $props();

  let busy = $state<null | "download" | "copy">(null);
  const copyable = canCopyImage();

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
