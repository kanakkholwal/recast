<script lang="ts" module>
  export interface DropZoneProps {
    /** Whether native screen capture is available (desktop only). */
    hasCapture: boolean;
    /** Whether an image is currently being dragged over the editor. */
    dragging: boolean;
    onupload: () => void;
    oncapture?: () => void;
  }
</script>

<script lang="ts">
  import { Button } from "@recast/ui/button";
  import { Camera, ImageUp, Clipboard } from "@lucide/svelte";

  let { hasCapture, dragging, onupload, oncapture }: DropZoneProps = $props();
</script>

<div
  class="flex h-full w-full flex-col items-center justify-center rounded-2xl border-2 border-dashed p-10 text-center transition-colors"
  class:border-primary={dragging}
  class:bg-primary={dragging}
  class:border-border={!dragging}
>
  <div class="bg-muted text-muted-foreground mb-5 flex size-14 items-center justify-center rounded-2xl">
    <ImageUp class="size-6" />
  </div>
  <h2 class="text-foreground text-lg font-semibold">Drop a screenshot to beautify it</h2>
  <p class="text-muted-foreground mt-1.5 max-w-sm text-sm">
    Drag an image here, paste from your clipboard, or pick a file. Everything stays on your
    device.
  </p>

  <div class="mt-6 flex flex-wrap items-center justify-center gap-2.5">
    {#if hasCapture}
      <Button variant="default" size="sm" onclick={() => oncapture?.()}>
        <Camera />
        Capture screen
      </Button>
    {/if}
    <Button variant={hasCapture ? "outline" : "default"} size="sm" onclick={onupload}>
      <ImageUp />
      Upload image
    </Button>
  </div>

  <p class="text-muted-foreground/80 mt-5 flex items-center gap-1.5 text-xs">
    <Clipboard class="size-3.5" />
    or press
    <kbd class="bg-muted rounded px-1.5 py-0.5 font-mono text-[11px]">Ctrl/Cmd + V</kbd>
    to paste
  </p>
</div>
