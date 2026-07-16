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

  /** Side-panel width bounds. The default IS the minimum: the controls are laid
   * out for 240px and get cramped below it. The ceiling is a quarter of the
   * viewport so the stage always keeps the majority of the screen. */
  const PANEL_MIN = 240;
  const PANEL_MAX_VW = 0.25;
  /** Pixels per arrow-key press on a separator. */
  const PANEL_STEP = 16;
</script>

<script lang="ts">
  import { Button } from "@recast/ui/button";
  import { cn } from "@recast/ui/utils";
  import * as Tabs from "@recast/ui/tabs";
  import * as Popover from "@recast/ui/popover";
  import { Segmented } from "@recast/ui/segmented";
  import {
    Box,
    Camera,
    Clapperboard,
    Copy,
    Download,
    Grid3x3,
    ImageUp,
    Layers,
    Palette,
    Ratio,
    Redo2,
    RotateCcw,
    Ruler,
    SlidersHorizontal,
    Trash2,
    Undo2,
    Wand2,
    X,
  } from "@recast/icons";
  import { ScreenshotEditorState } from "./editor.svelte";
  import { canCopyImage, copyToClipboard } from "./export";
  import { imageFromDataTransfer, imageFromFile, imageFromSrc } from "./image-input";
  import { captureWebsite } from "./website";
  import EditorStage from "./components/EditorStage.svelte";
  import DropZone from "./components/DropZone.svelte";
  import TimelineEditor from "./components/TimelineEditor.svelte";
  import TemplateControl from "./components/TemplateControl.svelte";
  import BackgroundControl from "./components/BackgroundControl.svelte";
  import MockupControl from "./components/MockupControl.svelte";
  import StyleControl from "./components/StyleControl.svelte";
  import BorderControl from "./components/BorderControl.svelte";
  import ShadowPresetControl from "./components/ShadowPresetControl.svelte";
  import FilterControl from "./components/FilterControl.svelte";
  import CanvasControl from "./components/CanvasControl.svelte";
  import LayerControl from "./components/LayerControl.svelte";
  import PerspectiveControl from "./components/PerspectiveControl.svelte";
  import ShadowControl from "./components/ShadowControl.svelte";
  import AnimationControl from "./components/AnimationControl.svelte";
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
  let showTemplates = $state(false);
  // Tab ids mirror the React app (edit/background/depth · transforms/animate).
  let leftTab = $state<"edit" | "background" | "depth">("edit");
  let rightTab = $state<"transforms" | "animate">("transforms");
  let editorMode = $state<"screenshot" | "browser">("screenshot");
  const dragging = $derived(dragDepth > 0);

  // --- Resizable side panels ------------------------------------------------
  // The default 240px is the FLOOR (the controls are laid out for it), and the
  // ceiling is a quarter of the viewport so the stage always keeps the majority
  // of the screen. Widths are per-session, not persisted.
  let viewportWidth = $state(1440);
  let leftWidth = $state(PANEL_MIN);
  let rightWidth = $state(PANEL_MIN);
  /** Never let the ceiling fall under the floor on a narrow window. */
  const panelMax = $derived(Math.max(PANEL_MIN, Math.round(viewportWidth * PANEL_MAX_VW)));

  const clampPanel = (px: number) => Math.max(PANEL_MIN, Math.min(panelMax, px));

  // Keep both panels legal if the window shrinks under them.
  $effect(() => {
    leftWidth = clampPanel(leftWidth);
    rightWidth = clampPanel(rightWidth);
  });

  let resizing = $state<"left" | "right" | null>(null);
  let dragStartX = 0;
  let dragStartWidth = 0;

  function startResize(side: "left" | "right", e: PointerEvent) {
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    resizing = side;
    dragStartX = e.clientX;
    dragStartWidth = side === "left" ? leftWidth : rightWidth;
  }

  function moveResize(e: PointerEvent) {
    if (!resizing) return;
    // The left panel grows as the pointer moves right; the right panel is
    // mirrored, so its delta is inverted.
    const delta = e.clientX - dragStartX;
    const next = clampPanel(dragStartWidth + (resizing === "left" ? delta : -delta));
    if (resizing === "left") leftWidth = next;
    else rightWidth = next;
  }

  function endResize() {
    resizing = null;
  }

  /** Keyboard resizing, so the separator is not pointer-only. */
  function resizeKey(side: "left" | "right", e: KeyboardEvent) {
    const width = side === "left" ? leftWidth : rightWidth;
    let next: number | null = null;
    if (e.key === "ArrowLeft") next = width + (side === "left" ? -PANEL_STEP : PANEL_STEP);
    else if (e.key === "ArrowRight") next = width + (side === "left" ? PANEL_STEP : -PANEL_STEP);
    else if (e.key === "Home") next = PANEL_MIN;
    else if (e.key === "End") next = panelMax;
    if (next === null) return;
    e.preventDefault();
    const clamped = clampPanel(next);
    if (side === "left") leftWidth = clamped;
    else rightWidth = clamped;
  }

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

  async function copyStage() {
    if (!stageEl) return;
    try {
      await copyToClipboard(stageEl, editor.exportScale);
      notify("Copied to clipboard", "success");
    } catch (e) {
      notify(e instanceof Error ? e.message : "Could not copy the image", "error");
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

<svelte:window onpaste={onPaste} onkeydown={onKeydown} bind:innerWidth={viewportWidth} />

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
  class:select-none={resizing !== null}
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
    <!-- HEADER (h-14) — mirrors the React "Stage" EditorHeader IA -->
    <header
      class="border-border/40 bg-card flex h-14 shrink-0 items-center justify-between border-b px-4"
    >
      <div class="flex items-center gap-1.5">
        <Button
          variant={showTemplates ? "secondary" : "ghost"}
          size="sm"
          onclick={() => (showTemplates = !showTemplates)}
        >
          <Wand2 />
          Templates
        </Button>
        <Button
          variant="ghost"
          size="icon"
          aria-label="Reset to defaults"
          title="Reset to defaults"
          onclick={() => editor.reset()}
        >
          <RotateCcw />
        </Button>
        <div class="bg-border/60 mx-0.5 h-4 w-px"></div>
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
        <div class="bg-border/60 mx-0.5 h-4 w-px"></div>
        <Button
          variant={editor.showRulers ? "secondary" : "ghost"}
          size="icon"
          aria-label="Toggle rulers"
          aria-pressed={editor.showRulers}
          title="Rulers (preview only)"
          onclick={() => editor.toggleRulers()}
        >
          <Ruler />
        </Button>
        <Button
          variant={editor.showGrid ? "secondary" : "ghost"}
          size="icon"
          aria-label="Toggle grid"
          aria-pressed={editor.showGrid}
          title="Grid (preview only)"
          onclick={() => editor.toggleGrid()}
        >
          <Grid3x3 />
        </Button>
      </div>

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

        <Popover.Root>
          <Popover.Trigger>
            {#snippet child({ props })}
              <Button {...props} variant="ghost" size="sm">
                <Ratio />
                {editor.aspect.label}
              </Button>
            {/snippet}
          </Popover.Trigger>
          <Popover.Content align="end" class="w-64 p-2">
            <AspectControl {editor} />
          </Popover.Content>
        </Popover.Root>

        {#if canCopyImage()}
          <Button variant="ghost" size="sm" onclick={copyStage}>
            <Copy />
            Copy
          </Button>
        {/if}

        <Popover.Root>
          <Popover.Trigger>
            {#snippet child({ props })}
              <Button {...props} size="sm">
                <Download />
                Save
              </Button>
            {/snippet}
          </Popover.Trigger>
          <Popover.Content align="end" class="w-64 p-3">
            <ExportControl {editor} getStage={() => stageEl} {onnotify} />
          </Popover.Content>
        </Popover.Root>

        <div class="bg-border/60 mx-0.5 h-5 w-px"></div>
        <Button variant="ghost" size="sm" onclick={() => editor.clear()}>
          <Trash2 />
          Remove
        </Button>
      </div>
    </header>

    <!-- BODY: Left panel (240) · Center stage · Right panel (240). Always a row
         so the center preview keeps its size even when the host area is narrow. -->
    <div class="flex min-h-0 flex-1 flex-row">
      <!-- LEFT PANEL -->
      <aside
        class="border-border/40 bg-card relative flex shrink-0 flex-col overflow-hidden border-r"
        style:width={`${leftWidth}px`}
      >
        <div class="px-2.5 pt-2.5 pb-1">
          <Segmented
            options={[
              { value: "screenshot", label: "Screenshot" },
              { value: "browser", label: "Browser" },
            ]}
            value={editorMode}
            onValueChange={(v) => (editorMode = v as typeof editorMode)}
            aria-label="Editor mode"
          />
        </div>
        <div class="border-border/30 border-b px-2.5 py-2.5">
          <Tabs.Root value={leftTab} onValueChange={(v) => (leftTab = v as typeof leftTab)}>
            <Tabs.List class="grid w-full grid-cols-3">
              <Tabs.Trigger value="edit"><SlidersHorizontal class="size-4" />Design</Tabs.Trigger>
              <Tabs.Trigger value="background"><Palette class="size-4" />BG</Tabs.Trigger>
              <Tabs.Trigger value="depth"><Layers class="size-4" />Layers</Tabs.Trigger>
            </Tabs.List>
          </Tabs.Root>
        </div>
        <div class="min-h-0 flex-1 space-y-4 overflow-y-auto px-3 py-3">
          {#if leftTab === "edit"}
            {#if editorMode === "browser"}
              <MockupControl {editor} />
            {:else}
              <StyleControl {editor} />
              <BorderControl {editor} />
            {/if}
            <ShadowPresetControl {editor} />
            <TextControl {editor} />
            <ShapeControl {editor} />
            <FilterControl {editor} />
            <CanvasControl {editor} />
          {:else if leftTab === "background"}
            <BackgroundControl {editor} />
          {:else}
            <LayerControl {editor} />
          {/if}
        </div>

        <!-- Templates overlay (slides in from the header button) -->
        <div
          class={cn(
            "bg-card absolute inset-0 z-40 flex flex-col transition-all duration-300 ease-out",
            showTemplates
              ? "translate-x-0 opacity-100"
              : "pointer-events-none -translate-x-full opacity-0",
          )}
        >
          <div class="border-border/30 flex items-center justify-between border-b px-3 py-3">
            <div class="flex items-center gap-2">
              <Wand2 class="text-primary size-4" />
              <h2 class="text-foreground text-sm font-semibold">Templates</h2>
            </div>
            <Button variant="ghost" size="icon" aria-label="Close templates" onclick={() => (showTemplates = false)}>
              <X />
            </Button>
          </div>
          <div class="min-h-0 flex-1 overflow-y-auto p-3">
            <TemplateControl {editor} />
          </div>
        </div>
      </aside>

      <!-- Drag handle between the left panel and the stage. `role="slider"` (the
           same pattern as @recast/ui's SliderControl) because it is a focusable
           widget carrying a value: an ARIA `separator` is treated as
           non-interactive and cannot own the keyboard. Arrows resize, Home/End
           jump to the bounds. -->
      <button
        type="button"
        class={cn(
          "hover:bg-primary/40 focus-visible:bg-primary/60 relative w-1 shrink-0 cursor-col-resize outline-none transition-colors",
          resizing === "left" ? "bg-primary/60" : "bg-transparent",
        )}
        role="slider"
        aria-label="Resize the left panel"
        aria-valuenow={leftWidth}
        aria-valuemin={PANEL_MIN}
        aria-valuemax={panelMax}
        aria-valuetext={`${leftWidth} pixels`}
        onpointerdown={(e) => startResize("left", e)}
        onpointermove={moveResize}
        onpointerup={endResize}
        onpointercancel={endResize}
        onkeydown={(e) => resizeKey("left", e)}
      ></button>

      <!-- CENTER: stage + floating Animate pill + bottom timeline -->
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

          {#if !showMotion}
            <button
              type="button"
              class="border-border/50 bg-card/90 text-muted-foreground hover:text-foreground hover:bg-card hover:border-border absolute bottom-4 left-1/2 z-20 flex -translate-x-1/2 items-center gap-2 rounded-full border px-5 py-2.5 shadow-lg backdrop-blur-md transition-all duration-200 ease-out hover:shadow-xl"
              onclick={() => (showMotion = true)}
            >
              <Clapperboard class="text-primary size-4" />
              <span class="text-sm font-medium">Animate</span>
            </button>
          {/if}
        </div>
        {#if showMotion}
          <TimelineEditor {editor} onclose={() => (showMotion = false)} />
        {/if}
      </div>

      <!-- Separator between the stage and the right panel. -->
      <button
        type="button"
        class={cn(
          "hover:bg-primary/40 focus-visible:bg-primary/60 relative w-1 shrink-0 cursor-col-resize outline-none transition-colors",
          resizing === "right" ? "bg-primary/60" : "bg-transparent",
        )}
        role="slider"
        aria-label="Resize the right panel"
        aria-valuenow={rightWidth}
        aria-valuemin={PANEL_MIN}
        aria-valuemax={panelMax}
        aria-valuetext={`${rightWidth} pixels`}
        onpointerdown={(e) => startResize("right", e)}
        onpointermove={moveResize}
        onpointerup={endResize}
        onpointercancel={endResize}
        onkeydown={(e) => resizeKey("right", e)}
      ></button>

      <!-- RIGHT PANEL -->
      <aside
        class="border-border/40 bg-card flex shrink-0 flex-col overflow-hidden border-l"
        style:width={`${rightWidth}px`}
      >
        <div class="border-border/30 border-b px-2.5 py-2.5">
          <Tabs.Root value={rightTab} onValueChange={(v) => (rightTab = v as typeof rightTab)}>
            <Tabs.List class="grid w-full grid-cols-2">
              <Tabs.Trigger value="transforms"><Box class="size-4" />3D</Tabs.Trigger>
              <Tabs.Trigger value="animate"><Clapperboard class="size-4" />Motion</Tabs.Trigger>
            </Tabs.List>
          </Tabs.Root>
        </div>
        <div class="min-h-0 flex-1 space-y-4 overflow-y-auto px-3 py-3">
          {#if rightTab === "transforms"}
            <PerspectiveControl {editor} />
            <ShadowControl {editor} />
          {:else}
            <AnimationControl {editor} />
          {/if}
        </div>
      </aside>
    </div>
  {/if}
</div>
