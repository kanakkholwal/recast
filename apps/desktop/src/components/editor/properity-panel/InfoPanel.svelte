<script lang="ts">
  import type {
    Annotation,
    AnnotationKindName,
    EditorStore,
    PanelTab,
  } from "$lib/stores/editor-store.svelte";
  import { clock } from "$lib/format/time";
  import { formatBytes as formatBytesBase } from "$lib/format/bytes";
  import { basename, countByKind, formatRelative } from "./info-panel.logic";
  import {
    ArrowUpRight,
    ChevronRight,
    Circle,
    Clock,
    Copy,
    Disc3,
    Film,
    FolderOpen,
    Gauge,
    HardDrive,
    ImageIcon,
    MousePointer,
    Pencil,
    Scissors,
    Square,
    Stamp,
    Target,
    Type as TypeIcon,
    Volume2,
    VolumeX,
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { toast } from "@recast/ui/sonner";
  import { openFileLocation } from "$lib/ipc";
  import { onDestroy, onMount } from "svelte";
  import PanelSection from "./PanelSection.svelte";

  interface Props {
    store: EditorStore;
  }

  let { store }: Props = $props();

  // Tick every 30s so relative-time labels stay fresh without a per-frame redraw.
  let now = $state(Date.now());
  let nowTimer: ReturnType<typeof setInterval> | null = null;
  onMount(() => {
    nowTimer = setInterval(() => (now = Date.now()), 30_000);
  });
  onDestroy(() => {
    if (nowTimer !== null) clearInterval(nowTimer);
  });

  function goTo(tab: PanelTab) {
    store.activePanel = tab;
  }

  // Keeps the "--:--" placeholder; defers clock formatting to the shared helper.
  function formatDuration(seconds: number | undefined): string {
    if (!seconds || seconds <= 0) return "--:--";
    return clock(seconds);
  }

  function formatResolution(): string {
    if (!store.metadata?.width || !store.metadata?.height) return "Unknown";
    return `${store.metadata.width}×${store.metadata.height}`;
  }

  function formatFps(): string {
    if (!store.metadata?.fps) return "--";
    return `${Math.round(store.metadata.fps)} fps`;
  }

  // Wrapper: InfoPanel shows "--" for missing sizes (vs the shared default "0 B").
  const formatBytes = (bytes: number | undefined): string =>
    formatBytesBase(bytes, "--");

  // Every kind always rendered (with 0) so the row doesn't shift as shapes change.
  const KIND_META: Array<{
    id: AnnotationKindName;
    label: string;
    icon: typeof Square;
  }> = [
    { id: "rect", label: "Rect", icon: Square },
    { id: "ellipse", label: "Ellipse", icon: Circle },
    { id: "arrow", label: "Arrow", icon: ArrowUpRight },
    { id: "text", label: "Text", icon: TypeIcon },
    { id: "image", label: "Image", icon: ImageIcon },
  ];

  const annotationCounts = $derived(countByKind(store.annotations));
  const totalAnnotations = $derived(store.annotations.length);

  const trimmed = $derived(
    store.metadata !== null &&
      (store.inPoint > 0 || store.outPoint < (store.metadata?.duration ?? 0)),
  );

  // Inline spec summary for the hero card: "1:24 · 1920×1080 · 60 fps".
  const specLine = $derived(
    [
      formatDuration(store.metadata?.duration),
      formatResolution(),
      formatFps(),
    ]
      .filter((s) => s && s !== "Unknown" && s !== "--" && s !== "--:--")
      .join(" · ") || "No metadata",
  );

  const cursorOn = $derived(store.cursorSettings.enabled);
  const muted = $derived(store.audioSettings?.muted ?? false);
  const audioValue = $derived(
    muted ? "Muted" : `${Math.round(store.audioSettings?.volume ?? 100)}%`,
  );

  // Saved-status pill summary.
  const saveStatus = $derived.by(() => {
    if (store.isDirty)
      return { label: "Unsaved changes", tone: "warning" } as const;
    if (store.lastSavedAt)
      return {
        label: `Saved ${formatRelative(store.lastSavedAt, now)}`,
        tone: "ok",
      } as const;
    return { label: "Not yet saved", tone: "muted" } as const;
  });

  async function copyToClipboard(text: string, label: string) {
    try {
      await navigator.clipboard.writeText(text);
      toast.success(`${label} copied`);
    } catch {
      toast.error("Could not copy to clipboard");
    }
  }

  async function revealInFolder(path: string) {
    if (!path) return;
    try {
      await openFileLocation(path);
    } catch (err) {
      const msg = typeof err === "string" ? err : String(err);
      toast.error(`Could not open folder: ${msg}`);
    }
  }
</script>

<!-- Read-only stat row: icon + label on the left, mono value on the right. -->
{#snippet stat(Icon: typeof Square, label: string, value: string)}
  <div class="flex items-center justify-between gap-2 px-1.5 py-1">
    <span class="flex items-center gap-1.5 text-muted-foreground">
      <Icon size={11} />
      {label}
    </span>
    <span class="font-mono tabular-nums text-foreground">{value}</span>
  </div>
{/snippet}

<!-- Actionable stat row: jumps to the related panel on click. -->
{#snippet navStat(Icon: typeof Square, label: string, value: string, tab: PanelTab)}
  <button
    type="button"
    onclick={() => goTo(tab)}
    class="group flex w-full items-center justify-between gap-2 rounded-md border border-transparent px-1.5 py-1 text-left transition-colors hover:border-border/60 hover:bg-card/60 focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
    title="Open {label} panel"
  >
    <span class="flex items-center gap-1.5 text-muted-foreground">
      <Icon size={11} />
      {label}
    </span>
    <span class="flex items-center gap-1">
      <span class="font-mono tabular-nums text-foreground">{value}</span>
      <ChevronRight
        size={12}
        class="text-muted-foreground/40 transition-transform group-hover:translate-x-0.5 group-hover:text-muted-foreground"
      />
    </span>
  </button>
{/snippet}

<div class="flex flex-col gap-4 text-xs animate-in fade-in duration-200">
  <!-- Hero: filename + key specs + live save status -->
  <div
    class="rounded-xl border border-border/60 bg-card/40 p-3 shadow-(--shadow-craft-inset)"
  >
    <div class="flex items-center gap-2">
      <span
        class="grid size-8 shrink-0 place-items-center rounded-lg bg-primary/10 text-primary"
        aria-hidden="true"
      >
        <Film size={15} />
      </span>
      <div class="min-w-0 flex-1">
        <p
          class="truncate font-mono text-[11px] font-medium text-foreground"
          title={store.videoPath}
        >
          {basename(store.videoPath)}
        </p>
        <p class="truncate text-[10px] tabular-nums text-muted-foreground">
          {specLine}
        </p>
      </div>
    </div>
    <div
      class="mt-2.5 flex items-center justify-center rounded-lg border border-border/50 bg-background/40 px-2 py-1"
    >
      <span
        class="inline-flex items-center gap-1.5 font-mono text-[10px] {saveStatus.tone ===
        'warning'
          ? 'text-warning'
          : saveStatus.tone === 'ok'
            ? 'text-success'
            : 'text-muted-foreground'}"
      >
        <span
          class="inline-flex size-1.5 rounded-full {saveStatus.tone === 'warning'
            ? 'bg-warning'
            : saveStatus.tone === 'ok'
              ? 'bg-success'
              : 'bg-muted-foreground'}"
          aria-hidden="true"
        ></span>
        {saveStatus.label}
      </span>
    </div>
  </div>

  <PanelSection title="Source" flush>
    <div class="flex flex-col gap-0.5">
      {@render stat(Clock, "Duration", formatDuration(store.metadata?.duration))}
      {@render stat(Film, "Resolution", formatResolution())}
      {@render stat(Gauge, "Frame rate", formatFps())}
      {@render stat(Disc3, "Codec", store.metadata?.codec || "—")}
      {@render stat(HardDrive, "File size", formatBytes(store.metadata?.sizeBytes))}
    </div>
  </PanelSection>

  <PanelSection title="Project" flush>
    <div class="flex flex-col gap-0.5">
      {@render stat(
        Scissors,
        "Trim",
        trimmed ? `${formatDuration(store.clipDuration)} kept` : "Full clip",
      )}
      {#if trimmed}
        <div class="flex items-center justify-between gap-2 px-1.5 py-1 pl-7">
          <span class="text-muted-foreground">In / Out</span>
          <span class="font-mono tabular-nums text-foreground">
            {formatDuration(store.inPoint)} → {formatDuration(store.outPoint)}
          </span>
        </div>
      {/if}
      {#if store.lastSavedAt}
        <div class="flex items-center justify-between gap-2 px-1.5 py-1">
          <span class="text-muted-foreground">Last saved</span>
          <span class="font-mono tabular-nums text-foreground">
            {new Date(store.lastSavedAt).toLocaleString()}
          </span>
        </div>
      {/if}
    </div>
  </PanelSection>

  <PanelSection title="Edits" flush>
    <div class="flex flex-col gap-0.5">
      {@render navStat(
        Target,
        "Focus regions",
        String(store.zoomRegions.length),
        "focus",
      )}
      {@render navStat(
        Pencil,
        "Annotations",
        String(totalAnnotations),
        "annotations",
      )}
      {#if totalAnnotations > 0}
        <div
          class="mx-1.5 grid grid-cols-5 gap-1 rounded-md border border-border/60 bg-background/40 p-1 shadow-(--shadow-craft-inset)"
        >
          {#each KIND_META as kind (kind.id)}
            {@const Icon = kind.icon}
            {@const count = annotationCounts[kind.id] ?? 0}
            <div
              class="flex flex-col items-center gap-0.5 rounded-sm px-1 py-1 {count >
              0
                ? 'bg-primary/8 text-primary ring-1 ring-primary/20'
                : 'text-muted-foreground/50'}"
              title="{kind.label}: {count}"
            >
              <Icon size={11} />
              <span class="font-mono text-[9px] tabular-nums">{count}</span>
            </div>
          {/each}
        </div>
      {/if}
      {@render navStat(
        MousePointer,
        "Cursor overlay",
        cursorOn ? "On" : "Off",
        "cursor",
      )}
      {@render navStat(muted ? VolumeX : Volume2, "Audio", audioValue, "audio")}
      {@render stat(
        Stamp,
        "Watermark",
        store.watermarkSettings?.enabled ? "On" : "Off",
      )}
    </div>
  </PanelSection>

  <PanelSection title="Files" flush>
    <div class="space-y-2">
      <div class="space-y-1">
        <div class="flex items-center justify-between gap-2">
          <span class="text-muted-foreground">Recording</span>
          <div class="flex items-center gap-0.5">
            <Button
              variant="ghost"
              size="icon-sm"
              onclick={() => copyToClipboard(store.videoPath, "Path")}
              aria-label="Copy recording path"
              title="Copy path"
            >
              <Copy size={11} />
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              onclick={() => revealInFolder(store.videoPath)}
              aria-label="Reveal in folder"
              title="Reveal in folder"
            >
              <FolderOpen size={11} />
            </Button>
          </div>
        </div>
        <p
          class="truncate rounded border border-border bg-background/60 px-1.5 py-1 font-mono text-[10px] text-foreground"
          title={store.videoPath}
        >
          {store.videoPath || "—"}
        </p>
      </div>
      {#if store.cursorPath}
        <div class="space-y-1">
          <div class="flex items-center justify-between gap-2">
            <span class="text-muted-foreground">Cursor track</span>
            <Button
              variant="ghost"
              size="icon-sm"
              onclick={() =>
                store.cursorPath &&
                copyToClipboard(store.cursorPath, "Cursor path")}
              aria-label="Copy cursor track path"
              title="Copy path"
            >
              <Copy size={11} />
            </Button>
          </div>
          <p
            class="truncate rounded border border-border bg-background/60 px-1.5 py-1 font-mono text-[10px] text-foreground"
            title={store.cursorPath}
          >
            {store.cursorPath}
          </p>
        </div>
      {/if}
    </div>
  </PanelSection>
</div>
