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
  import {
    Camera,
    Clapperboard,
    ImageUp,
    Layers,
    Palette,
    Redo2,
    RotateCcw,
    SlidersHorizontal,
    Undo2,
    X,
  } from "@lucide/svelte";
  import { ScreenshotEditorState } from "./editor.svelte";
  import { imageFromDataTransfer, imageFromFile, imageFromSrc } from "./image-input";
  import { captureWebsite } from "./website";
  import EditorStage from "./components/EditorStage.svelte";
  import DropZone from "./components/DropZone.svelte";
  import MotionBar from "./components/MotionBar.svelte";
  import TemplateControl from "./components/TemplateControl.svelte";
  import BackgroundControl from "./components/BackgroundControl.svelte";
  import MockupControl from "./components/MockupControl.svelte";
  import FrameControl from "./components/FrameControl.svelte";
  import ShadowControl from "./components/ShadowControl.svelte";
  import PerspectiveControl from "./components/PerspectiveControl.svelte";
  import TextControl from "./components/TextControl.svelte";
  import ShapeControl from "./components/ShapeControl.svelte";
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
  let urlBusy = $state(false);
  let showMotion = $state(false);
  let leftTab = $state<"design" | "background" | "layers">("design");
  const dragging = $derived(dragDepth > 0);

  const LEFT_TABS = [
    { id: "design" as const, label: "Design", icon: SlidersHorizontal },
    { id: "background" as const, label: "BG", icon: Palette },
    { id: "layers" as const, label: "3D", icon: Layers },
  ];

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

  async function captureUrl(target: string) {
    if (urlBusy) return;
    urlBusy = true;
    try {
      editor.setImage(await captureWebsite(target));
    } catch (e) {
      notify(e instanceof Error ? e.message : "Could not capture that URL", "error");
    } finally {
      urlBusy = false;
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

  // Single source of history: any design change re-runs this and records a step.
  $effect(() => {
    editor.record();
  });

  // Animation playback clock: (re)starts whenever `playing` flips on.
  $effect(() => {
    if (!editor.playing) return;
    let raf = 0;
    let last = performance.now();
    const step = (now: number) => {
      editor.advance(now - last);
      last = now;
      raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  });

  function onKeydown(e: KeyboardEvent) {
    if (!(e.ctrlKey || e.metaKey)) return;
    const t = e.target as HTMLElement | null;
    // Let inputs keep native text undo.
    if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable)) return;
    const key = e.key.toLowerCase();
    if (key === "z" && !e.shiftKey) {
      e.preventDefault();
      editor.undo();
    } else if ((key === "z" && e.shiftKey) || key === "y") {
      e.preventDefault();
      editor.redo();
    }
  }
</script>

<svelte:window onpaste={onPaste} onkeydown={onKeydown} />

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
      <DropZone
        {dragging}
        {urlBusy}
        hasCapture={!!oncapture}
        onupload={openFilePicker}
        oncapture={capture}
        onwebsite={captureUrl}
      />
    </div>
  {:else}
    <!-- Top toolbar -->
    <div class="border-border flex items-center justify-between gap-2 border-b px-4 py-2.5">
      <div class="flex items-center gap-1">
        <Button
          variant="ghost"
          size="icon"
          aria-label="Undo"
          title="Undo (Ctrl/Cmd+Z)"
          disabled={!editor.canUndo}
          onclick={() => editor.undo()}
        >
          <Undo2 />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          aria-label="Redo"
          title="Redo (Ctrl/Cmd+Shift+Z)"
          disabled={!editor.canRedo}
          onclick={() => editor.redo()}
        >
          <Redo2 />
        </Button>
      </div>
      <div class="flex items-center gap-1.5">
        <Button
          variant={showMotion ? "secondary" : "ghost"}
          size="sm"
          onclick={() => (showMotion = !showMotion)}
        >
          <Clapperboard />
          Animate
        </Button>
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

    <!-- Left tools · Center stage · Right settings. Always a row so the center
         preview keeps its width/height even when the host area is narrow. -->
    <div class="flex min-h-0 flex-1 flex-row">
      <!-- LEFT: design tools (tabbed) -->
      <aside
        class="border-border bg-background flex w-60 shrink-0 flex-col border-r"
      >
        <div class="bg-muted/50 m-3 flex gap-1 rounded-lg p-0.5">
          {#each LEFT_TABS as tab (tab.id)}
            {@const Icon = tab.icon}
            <button
              type="button"
              class="flex flex-1 items-center justify-center gap-1.5 rounded-md py-1.5 text-xs font-medium transition"
              class:bg-background={leftTab === tab.id}
              class:text-foreground={leftTab === tab.id}
              class:shadow-sm={leftTab === tab.id}
              class:text-muted-foreground={leftTab !== tab.id}
              onclick={() => (leftTab = tab.id)}
            >
              <Icon class="size-3.5" />
              {tab.label}
            </button>
          {/each}
        </div>
        <div class="min-h-0 flex-1 space-y-4 overflow-y-auto px-3 pb-4">
          {#if leftTab === "design"}
            <TemplateControl {editor} />
            <MockupControl {editor} />
            <FrameControl {editor} />
            <ShadowControl {editor} />
            <TextControl {editor} />
            <ShapeControl {editor} />
          {:else if leftTab === "background"}
            <BackgroundControl {editor} />
          {:else}
            <PerspectiveControl {editor} />
          {/if}
        </div>
      </aside>

      <!-- CENTER: stage + motion timeline -->
      <div class="flex min-h-0 min-w-0 flex-1 flex-col">
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
        {#if showMotion}
          <MotionBar {editor} onclose={() => (showMotion = false)} />
        {/if}
      </div>

      <!-- RIGHT: canvas + export settings -->
      <aside
        class="border-border bg-background w-60 shrink-0 space-y-4 overflow-y-auto border-l p-3"
      >
        <AspectControl {editor} />
        <ExportControl {editor} getStage={() => stageEl} {onnotify} />
      </aside>
    </div>
  {/if}
</div>
