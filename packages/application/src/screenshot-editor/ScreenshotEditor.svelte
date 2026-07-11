<script lang="ts" module>
  import type { EditorImage } from "./types";

  export interface ScreenshotEditorProps {
    /** Reuse an existing session, or let the editor own one. */
    editor?: ScreenshotEditorState;
    /** Native screen capture (desktop). Omit on web to offer upload/paste only. */
    oncapture?: () => Promise<EditorImage | null>;
    /** App hook for toasts; falls back to console on error. */
    onnotify?: (message: string, kind: "success" | "error") => void;
    class?: string;
  }
</script>

<script lang="ts">
  import { Button } from "@recast/ui/button";
  import { cn } from "@recast/ui/utils";
  import { Camera, ImageUp, RotateCcw, X } from "@lucide/svelte";
  import { ScreenshotEditorState } from "./editor.svelte";
  import { imageFromDataTransfer, imageFromFile, imageFromSrc } from "./image-input";
  import EditorStage from "./components/EditorStage.svelte";
  import DropZone from "./components/DropZone.svelte";
  import BackgroundControl from "./components/BackgroundControl.svelte";
  import MockupControl from "./components/MockupControl.svelte";
  import FrameControl from "./components/FrameControl.svelte";
  import AspectControl from "./components/AspectControl.svelte";
  import ExportControl from "./components/ExportControl.svelte";

  let {
    editor = new ScreenshotEditorState(),
    oncapture,
    onnotify,
    class: className,
  }: ScreenshotEditorProps = $props();

  let stageEl = $state<HTMLElement | null>(null);
  let fileInput = $state<HTMLInputElement | null>(null);
  let dragDepth = $state(0);
  const dragging = $derived(dragDepth > 0);

  function notify(message: string, kind: "success" | "error") {
    if (onnotify) onnotify(message, kind);
    else if (kind === "error") console.error(message);
  }

  async function loadFile(file: Blob) {
    try {
      editor.setImage(await imageFromFile(file));
    } catch (e) {
      notify(e instanceof Error ? e.message : "Could not load that image", "error");
    }
  }

  function openFilePicker() {
    fileInput?.click();
  }

  function onFileChosen(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (file) void loadFile(file);
    input.value = ""; // allow re-picking the same file
  }

  async function capture() {
    if (!oncapture) return;
    try {
      const img = await oncapture();
      if (img) editor.setImage(await imageFromSrc(img.src));
    } catch (e) {
      notify(e instanceof Error ? e.message : "Capture failed", "error");
    }
  }

  // Paste an image from anywhere in the editor.
  function onPaste(e: ClipboardEvent) {
    const file = imageFromDataTransfer(e.clipboardData);
    if (file) {
      e.preventDefault();
      void loadFile(file);
    }
  }

  function onDrop(e: DragEvent) {
    e.preventDefault();
    dragDepth = 0;
    const file = imageFromDataTransfer(e.dataTransfer);
    if (file) void loadFile(file);
  }
</script>

<svelte:window onpaste={onPaste} />

<input
  bind:this={fileInput}
  type="file"
  accept="image/*"
  class="hidden"
  onchange={onFileChosen}
/>

<div
  class={cn("bg-background flex h-full min-h-0 w-full flex-col", className)}
  ondragenter={(e) => {
    e.preventDefault();
    dragDepth += 1;
  }}
  ondragleave={() => {
    dragDepth = Math.max(0, dragDepth - 1);
  }}
  ondragover={(e) => e.preventDefault()}
  ondrop={onDrop}
  role="application"
  aria-label="Screenshot editor"
>
  {#if !editor.hasImage}
    <div class="flex min-h-0 flex-1 p-4 sm:p-6">
      <DropZone {dragging} hasCapture={!!oncapture} onupload={openFilePicker} oncapture={capture} />
    </div>
  {:else}
    <!-- Top toolbar -->
    <div class="border-border flex items-center justify-between gap-2 border-b px-4 py-2.5">
      <span class="text-muted-foreground text-sm font-medium">Screenshot editor</span>
      <div class="flex items-center gap-1.5">
        {#if oncapture}
          <Button variant="ghost" size="sm" onclick={capture}>
            <Camera />
            Recapture
          </Button>
        {/if}
        <Button variant="ghost" size="sm" onclick={openFilePicker}>
          <ImageUp />
          Replace
        </Button>
        <Button variant="ghost" size="sm" onclick={() => editor.reset()}>
          <RotateCcw />
          Reset
        </Button>
        <Button variant="ghost" size="icon" aria-label="Remove image" onclick={() => editor.clear()}>
          <X />
        </Button>
      </div>
    </div>

    <!-- Stage + controls -->
    <div class="flex min-h-0 flex-1 flex-col lg:flex-row">
      <div
        class="bg-muted/30 relative flex min-h-0 flex-1 items-center justify-center overflow-auto p-6 sm:p-10"
      >
        {#if dragging}
          <div
            class="border-primary bg-primary/5 text-primary pointer-events-none absolute inset-3 z-10 flex items-center justify-center rounded-2xl border-2 border-dashed text-sm font-medium"
          >
            Drop to replace
          </div>
        {/if}
        <div class="max-h-full w-full max-w-3xl">
          <EditorStage {editor} bind:stageEl />
        </div>
      </div>

      <aside
        class="border-border bg-background w-full shrink-0 space-y-4 overflow-y-auto border-t p-4 lg:w-80 lg:border-t-0 lg:border-l"
      >
        <BackgroundControl {editor} />
        <MockupControl {editor} />
        <FrameControl {editor} />
        <AspectControl {editor} />
        <ExportControl {editor} getStage={() => stageEl} {onnotify} />
      </aside>
    </div>
  {/if}
</div>
