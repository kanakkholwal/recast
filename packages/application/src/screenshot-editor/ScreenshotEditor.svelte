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

/** Side-panel width bounds. The default IS the minimum: the labelled PropRows
 * (w-20 label + control) get cramped below 288px. The ceiling is a quarter of
 * the viewport so the stage always keeps the majority of the screen. */
const PANEL_MIN = 288;
const PANEL_MAX_VW = 0.28;
/** Pixels per arrow-key press on a separator. */
const PANEL_STEP = 16;
</script>

<script lang="ts">
  import { Button } from "@recast/ui/button";
  import { cn } from "@recast/ui/utils";
  import * as Tabs from "@recast/ui/tabs";
  import * as Popover from "@recast/ui/popover";
  import * as Select from "@recast/ui/select";
  import * as Dialog from "@recast/ui/dialog";
  import { Segmented } from "@recast/ui/segmented";
  import { PanelSection } from "@recast/ui/panel-section";
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
    Redo2,
    RotateCcw,
    Ruler,
    SlidersHorizontal,
    Trash2,
    Undo2,
    Wand2,
    X,
  } from "@recast/icons";
  import { onMount } from "svelte";
  import { ScreenshotEditorState } from "./editor.svelte";
  import { clearDraft, loadDraft, saveDraft } from "./persistence";
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
  import TransformPad from "./components/TransformPad.svelte";
  import TransformsGallery from "./components/TransformsGallery.svelte";
  import ShadowControl from "./components/ShadowControl.svelte";
  import AnimationControl from "./components/AnimationControl.svelte";
  import TextControl from "./components/TextControl.svelte";
  import ShapeControl from "./components/ShapeControl.svelte";
  import OverlayControl from "./components/OverlayControl.svelte";
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
  // One inspector rail (video-editor pattern); content sections, then transform sections.
  const TABS = {
    edit: { label: "Design", hint: "Frame, style, text", icon: SlidersHorizontal },
    background: { label: "Background", hint: "Backdrop & canvas", icon: Palette },
    depth: { label: "Layers", hint: "Text & shape layers", icon: Layers },
    transforms: { label: "3D", hint: "Perspective & tilt", icon: Box },
    animate: { label: "Motion", hint: "Animate the shot", icon: Clapperboard },
  } as const;
  type TabId = keyof typeof TABS;
  const CONTENT_TABS: TabId[] = ["edit", "background", "depth"];
  const TRANSFORM_TABS: TabId[] = ["transforms", "animate"];
  let activeTab = $state<TabId>("edit");
  const activeMeta = $derived(TABS[activeTab]);
  let editorMode = $state<"screenshot" | "browser">("screenshot");
  const dragging = $derived(dragDepth > 0);

  // --- Resizable inspector: 288px floor, ~28% viewport ceiling. Per-session, not persisted.
  let viewportWidth = $state(1440);
  let panelWidth = $state(PANEL_MIN);
  const panelMax = $derived(Math.max(PANEL_MIN, Math.round(viewportWidth * PANEL_MAX_VW)));
  const clampPanel = (px: number) => Math.max(PANEL_MIN, Math.min(panelMax, px));

  // Keep the panel legal if the window shrinks under it.
  $effect(() => {
    panelWidth = clampPanel(panelWidth);
  });

  let resizing = $state(false);
  let dragStartX = 0;
  let dragStartWidth = 0;

  function startResize(e: PointerEvent) {
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    resizing = true;
    dragStartX = e.clientX;
    dragStartWidth = panelWidth;
  }

  function moveResize(e: PointerEvent) {
    if (!resizing) return;
    // Handle sits left of the panel, so dragging left grows it: invert the delta.
    panelWidth = clampPanel(dragStartWidth - (e.clientX - dragStartX));
  }

  function endResize() {
    resizing = false;
  }

  /** Keyboard resizing, so the separator is not pointer-only. */
  function resizeKey(e: KeyboardEvent) {
    let next: number | null = null;
    if (e.key === "ArrowLeft") next = panelWidth + PANEL_STEP;
    else if (e.key === "ArrowRight") next = panelWidth - PANEL_STEP;
    else if (e.key === "Home") next = PANEL_MIN;
    else if (e.key === "End") next = panelMax;
    if (next === null) return;
    e.preventDefault();
    panelWidth = clampPanel(next);
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

  // Restore once on mount, then debounce-save the full snapshot to IndexedDB so a refresh never loses work.
  let draftStatus = $state<"idle" | "saving" | "saved">("idle");
  let draftLoaded = $state(false);

  onMount(() => {
    void loadDraft().then((snap) => {
      if (snap?.image && !editor.hasImage) {
        editor.loadSnapshot(snap);
        draftStatus = "saved";
      }
      draftLoaded = true;
    });
  });

  $effect(() => {
    // Touching each persisted top-level ref subscribes cheaply (patch* replaces objects) without serializing per change.
    void editor.image;
    void editor.slides;
    void editor.activeSlide;
    void editor.keyframes;
    void editor.background;
    void editor.backgroundId;
    void editor.frame;
    void editor.shadow;
    void editor.imageStyle;
    void editor.mockup;
    void editor.transform;
    void editor.aspect;
    void editor.overlays;
    void editor.filters;
    void editor.imageScale;
    void editor.imageOpacity;
    void editor.backgroundBlur;
    void editor.backgroundNoise;
    void editor.canvasRadius;
    void editor.exportFormat;
    void editor.exportScale;
    void editor.exportQuality;
    if (!draftLoaded || !editor.hasImage) return;
    draftStatus = "saving";
    const id = setTimeout(() => {
      void saveDraft(editor.toSnapshot()).then(() => (draftStatus = "saved"));
    }, 800);
    return () => clearTimeout(id);
  });

  let showClearConfirm = $state(false);

  // Wipes the image and the saved draft, so confirm first: there is no undo past clear.
  function clearWorkspace() {
    showClearConfirm = false;
    editor.clear();
    void clearDraft();
    draftStatus = "idle";
  }

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
  class:select-none={resizing}
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
      class="border-border bg-card flex h-14 shrink-0 items-center justify-between border-b px-4"
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
        <div class="bg-border mx-0.5 h-4 w-px"></div>
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
        <div class="bg-border mx-0.5 h-4 w-px"></div>
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
        {#if editor.showRulers}
          <Select.Root
            type="single"
            value={String(editor.rulerInterval)}
            onValueChange={(v) => editor.setRulerInterval(Number(v))}
          >
            <Select.Trigger class="h-8 w-[4.5rem] text-xs" aria-label="Ruler interval" title="Ruler interval">
              {editor.rulerInterval}px
            </Select.Trigger>
            <Select.Content>
              {#each [25, 50, 100, 200] as step (step)}
                <Select.Item value={String(step)}>{step}px</Select.Item>
              {/each}
            </Select.Content>
          </Select.Root>
        {/if}
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

        <AspectControl {editor} />

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
          <Popover.Content
            align="end"
            class="scrollbar-transparent max-h-[min(70vh,540px)] w-72 space-y-4 overflow-y-auto p-3"
          >
            <ExportControl {editor} getStage={() => stageEl} {onnotify} />
          </Popover.Content>
        </Popover.Root>

        {#if draftStatus !== "idle"}
          <span class="text-muted-foreground hidden text-xs sm:inline" aria-live="polite">
            {draftStatus === "saving" ? "Saving…" : "Saved"}
          </span>
        {/if}

        <div class="bg-border mx-0.5 h-5 w-px"></div>
        <Button variant="ghost" size="sm" onclick={() => (showClearConfirm = true)}>
          <Trash2 />
          Remove
        </Button>
      </div>
    </header>

    <!-- BODY: the stage fills the remaining space; one inspector rail on the right. -->
    <div class="flex min-h-0 flex-1 flex-row">
      <!-- CENTER: stage + floating Animate pill + bottom timeline -->
      <div class="flex min-h-0 min-w-0 flex-1 flex-col">
        <div
          class="bg-muted scrollbar-transparent relative flex min-h-0 flex-1 items-center justify-center overflow-auto p-6 sm:p-10"
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
              class="border-border bg-card text-muted-foreground hover:text-foreground hover:bg-accent absolute bottom-4 left-1/2 z-20 flex -translate-x-1/2 items-center gap-2 rounded-full border px-4 py-2 text-sm font-medium shadow-craft-sm transition-colors duration-200 active:scale-[0.99]"
              onclick={() => (showMotion = true)}
            >
              <Clapperboard class="size-4" />
              Animate
            </button>
          {/if}
        </div>
        {#if showMotion}
          <TimelineEditor {editor} onclose={() => (showMotion = false)} />
        {/if}
      </div>

      <!-- Resize handle. `role="slider"` (not `separator`, which ARIA treats as
           non-interactive) so it owns the keyboard: arrows resize, Home/End jump. -->
      <button
        type="button"
        class="group/resize relative w-2 shrink-0 cursor-col-resize outline-none"
        role="slider"
        aria-label="Resize the inspector panel"
        aria-valuenow={panelWidth}
        aria-valuemin={PANEL_MIN}
        aria-valuemax={panelMax}
        aria-valuetext={`${panelWidth} pixels`}
        onpointerdown={startResize}
        onpointermove={moveResize}
        onpointerup={endResize}
        onpointercancel={endResize}
        onkeydown={resizeKey}
      >
        <span
          class={cn(
            "pointer-events-none absolute inset-y-0 left-1/2 w-px -translate-x-1/2 transition-colors",
            resizing
              ? "bg-primary"
              : "bg-transparent group-hover/resize:bg-primary/60 group-focus-visible/resize:bg-primary",
          )}
        ></span>
      </button>

      <!-- INSPECTOR: one rail with every section (video-editor PropertiesPanel). -->
      <aside
        class="border-border bg-card relative flex shrink-0 flex-col overflow-hidden border-l"
        style:width={`${panelWidth}px`}
      >
        <!-- Icon rail: content sections · divider · transform sections. -->
        <div class="border-border border-b px-2 py-1.5">
          <Tabs.Root value={activeTab} onValueChange={(v) => (activeTab = v as TabId)}>
            <Tabs.List variant="soft" class="w-full gap-0.5 bg-transparent">
              {#each CONTENT_TABS as id (id)}
                {@const T = TABS[id]}
                {@const Icon = T.icon}
                <Tabs.Trigger value={id} title={T.label} aria-label={T.label} class="size-8 flex-none px-0">
                  <Icon class="size-4" />
                </Tabs.Trigger>
              {/each}
              <div class="bg-border mx-1 h-4 w-px self-center"></div>
              {#each TRANSFORM_TABS as id (id)}
                {@const T = TABS[id]}
                {@const Icon = T.icon}
                <Tabs.Trigger value={id} title={T.label} aria-label={T.label} class="size-8 flex-none px-0">
                  <Icon class="size-4" />
                </Tabs.Trigger>
              {/each}
            </Tabs.List>
          </Tabs.Root>
        </div>
        <div class="border-border flex h-9 shrink-0 items-center justify-between gap-2 border-b px-3">
          <h2 class="text-foreground text-[13px] font-semibold tracking-tight">{activeMeta.label}</h2>
          <p class="text-muted-foreground truncate text-[11px]">{activeMeta.hint}</p>
        </div>
        <div class="scrollbar-transparent min-h-0 flex-1 space-y-4 overflow-y-auto px-3 py-3">
          {#if activeTab === "edit"}
            <PanelSection variant="panel" title="Frame">
              <Segmented
                options={[
                  { value: "screenshot", label: "Screenshot" },
                  { value: "browser", label: "Browser" },
                ]}
                value={editorMode}
                onValueChange={(v) => (editorMode = v as typeof editorMode)}
                aria-label="Frame type"
              />
            </PanelSection>
            {#if editorMode === "browser"}
              <MockupControl {editor} />
            {:else}
              <StyleControl {editor} />
              <BorderControl {editor} />
            {/if}
            <ShadowPresetControl {editor} />
            <OverlayControl {editor} />
            <ShapeControl {editor} />
            <TextControl {editor} />
            <FilterControl {editor} />
            <CanvasControl {editor} />
          {:else if activeTab === "background"}
            <BackgroundControl {editor} />
          {:else if activeTab === "depth"}
            <LayerControl {editor} />
          {:else if activeTab === "transforms"}
            <TransformPad {editor} />
            <TransformsGallery {editor} />
            <PerspectiveControl {editor} />
            <ShadowControl {editor} />
          {:else}
            <AnimationControl {editor} />
          {/if}
        </div>

        <!-- Templates overlay (slides in from the header Templates button). -->
        <div
          class={cn(
            "bg-card absolute inset-0 z-40 flex flex-col transition-[transform,opacity] duration-300 ease-[cubic-bezier(0.625,0.05,0,1)] motion-reduce:transition-none",
            showTemplates
              ? "translate-x-0 opacity-100"
              : "pointer-events-none translate-x-full opacity-0",
          )}
        >
          <div class="border-border flex items-center justify-between border-b px-3 py-3">
            <div class="flex items-center gap-2">
              <Wand2 class="text-muted-foreground size-4" />
              <h2 class="text-foreground text-sm font-semibold">Templates</h2>
            </div>
            <Button variant="ghost" size="icon" aria-label="Close templates" onclick={() => (showTemplates = false)}>
              <X />
            </Button>
          </div>
          <div class="scrollbar-transparent min-h-0 flex-1 overflow-y-auto p-3">
            <TemplateControl {editor} />
          </div>
        </div>
      </aside>
    </div>

    <Dialog.Root bind:open={showClearConfirm}>
      <Dialog.Content class="max-w-sm">
        <Dialog.Header>
          <Dialog.Title>Remove this screenshot?</Dialog.Title>
          <Dialog.Description>
            This clears the image and the saved draft. It cannot be undone.
          </Dialog.Description>
        </Dialog.Header>
        <Dialog.Footer>
          <Button variant="ghost" size="sm" onclick={() => (showClearConfirm = false)}>Cancel</Button>
          <Button variant="destructive" size="sm" onclick={clearWorkspace}>
            <Trash2 />
            Remove
          </Button>
        </Dialog.Footer>
      </Dialog.Content>
    </Dialog.Root>
  {/if}
</div>
