<script lang="ts">
  import type {
    Annotation,
    AnnotationKindName,
    EditorStore
  } from "$lib/stores/editor-store.svelte";
  import {
    ArrowUpRight,
    Circle,
    Clock,
    Copy,
    Disc3,
    FileText,
    Film,
    FolderOpen,
    Gauge,
    HardDrive,
    ImageIcon,
    MousePointer,
    Pencil,
    Save,
    Scissors,
    Square,
    Stamp,
    Type as TypeIcon,
    Video,
    Volume2,
    VolumeX
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { toast } from "@recast/ui/sonner";
  import { invoke } from "@tauri-apps/api/core";
  import { onDestroy, onMount } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { fly, scale } from "svelte/transition";

  interface Props {
    store: EditorStore;
  }


  let { store }: Props = $props();

  // When a zoom region is selected from the timeline, switch to the Focus tab
  // so the user lands on the relevant editor.
  $effect(() => {
    if (store.selectedZoomRegionId) {
      store.activePanel = "focus";
    }
  });
  $effect(() => {
    if (store.selectedAnnotationId || store.annotationTool) {
      store.activePanel = "annotations";
    }
  });

  // Tick `now` every 30s so the relative-time labels ("Saved 2 min ago")
  // stay fresh without paying for a per-frame redraw.
  let now = $state(Date.now());
  let nowTimer: ReturnType<typeof setInterval> | null = null;
  onMount(() => {
    nowTimer = setInterval(() => (now = Date.now()), 30_000);
  });
  onDestroy(() => {
    if (nowTimer !== null) clearInterval(nowTimer);
  });

  function formatDuration(seconds: number | undefined): string {
    if (!seconds || seconds <= 0) return "--:--";
    const t = Math.max(0, seconds);
    const m = Math.floor(t / 60);
    const s = Math.floor(t % 60);
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  function formatResolution(): string {
    if (!store.metadata?.width || !store.metadata?.height) return "Unknown";
    return `${store.metadata.width}×${store.metadata.height}`;
  }

  function formatFps(): string {
    if (!store.metadata?.fps) return "--";
    return `${Math.round(store.metadata.fps)} fps`;
  }

  /** Human-readable bytes: 1.4 GB, 932 MB, 84 KB. Defaults to "--" on 0/missing. */
  function formatBytes(bytes: number | undefined): string {
    if (!bytes || bytes <= 0) return "--";
    const units = ["B", "KB", "MB", "GB", "TB"];
    let i = 0;
    let v = bytes;
    while (v >= 1024 && i < units.length - 1) {
      v /= 1024;
      i++;
    }
    return `${v < 10 && i > 0 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
  }

  /** "in 2 min", "5 sec ago", "3 hr ago". Floors aggressively at the cutoffs
   *  used in chat-style UIs so the readout doesn't bounce by 1 unit each tick. */
  function formatRelative(ts: number | null, current: number): string {
    if (!ts) return "Never";
    const diffMs = current - ts;
    const future = diffMs < 0;
    const seconds = Math.floor(Math.abs(diffMs) / 1000);
    const minutes = Math.floor(seconds / 60);
    const hours = Math.floor(minutes / 60);
    const days = Math.floor(hours / 24);
    let label: string;
    if (seconds < 5) label = "just now";
    else if (seconds < 60) label = `${seconds}s`;
    else if (minutes < 60) label = `${minutes} min`;
    else if (hours < 24) label = `${hours} hr`;
    else label = `${days} day${days === 1 ? "" : "s"}`;
    if (label === "just now") return label;
    return future ? `in ${label}` : `${label} ago`;
  }

  function basename(path: string): string {
    if (!path) return "—";
    const sep = path.includes("\\") ? "\\" : "/";
    const last = path.split(sep).filter(Boolean).pop() ?? path;
    return last;
  }

  // Annotation kind counts. Always render every kind (with 0) so the row
  // doesn't shift around as the user adds/removes shapes.
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

  function countByKind(annotations: Annotation[]): Record<string, number> {
    const out: Record<string, number> = {
      rect: 0,
      ellipse: 0,
      arrow: 0,
      text: 0,
      image: 0,
    };
    for (const a of annotations) out[a.kind.kind] = (out[a.kind.kind] ?? 0) + 1;
    return out;
  }

  const annotationCounts = $derived(countByKind(store.annotations));
  const totalAnnotations = $derived(store.annotations.length);

  const trimmed = $derived(
    store.metadata !== null &&
      (store.inPoint > 0 || store.outPoint < (store.metadata?.duration ?? 0)),
  );

  // Saved-status pill summary.
  const saveStatus = $derived.by(() => {
    if (store.isDirty) return { label: "Unsaved changes", tone: "warning" } as const;
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
      await invoke("open_file_location", { path });
    } catch (err) {
      const msg = typeof err === "string" ? err : String(err);
      toast.error(`Could not open folder: ${msg}`);
    }
  }
</script>

<div class="flex flex-col gap-3 text-xs">
  <!-- Source section -->
  <section
    in:fly={{ y: 8, duration: 260, delay: 40, easing: cubicOut }}
    class="overflow-hidden rounded-xl border border-border/60 bg-card/70 shadow-(--shadow-craft-inset) backdrop-blur"
  >
    <header
      class="flex items-center justify-between gap-2 border-b border-border/60 px-2.5 py-1.5"
    >
      <div class="flex items-center gap-1.5 text-muted-foreground/80">
        <Video size={11} />
        <span class="text-[10px] font-bold uppercase tracking-[0.15em]">
          Source
        </span>
      </div>
      <span
        class="truncate font-mono text-[10px] text-foreground"
        title={store.videoPath}
      >
        {basename(store.videoPath)}
      </span>
    </header>
    <dl class="grid grid-cols-2 gap-x-2 gap-y-1.5 px-2.5 py-2">
      <div class="flex items-center gap-1.5 text-muted-foreground">
        <Clock size={10} />
        <span>Duration</span>
      </div>
      <dd class="text-right font-mono tabular-nums text-foreground">
        {formatDuration(store.metadata?.duration)}
      </dd>

      <div class="flex items-center gap-1.5 text-muted-foreground">
        <Film size={10} />
        <span>Resolution</span>
      </div>
      <dd class="text-right font-mono tabular-nums text-foreground">
        {formatResolution()}
      </dd>

      <div class="flex items-center gap-1.5 text-muted-foreground">
        <Gauge size={10} />
        <span>Frame rate</span>
      </div>
      <dd class="text-right font-mono tabular-nums text-foreground">
        {formatFps()}
      </dd>

      <div class="flex items-center gap-1.5 text-muted-foreground">
        <Disc3 size={10} />
        <span>Codec</span>
      </div>
      <dd class="text-right font-mono tabular-nums text-foreground">
        {store.metadata?.codec || "—"}
      </dd>

      <div class="flex items-center gap-1.5 text-muted-foreground">
        <HardDrive size={10} />
        <span>File size</span>
      </div>
      <dd class="text-right font-mono tabular-nums text-foreground">
        {formatBytes(store.metadata?.sizeBytes)}
      </dd>
    </dl>
  </section>

  <!-- Project / save state -->
  <section
    in:fly={{ y: 8, duration: 260, delay: 100, easing: cubicOut }}
    class="overflow-hidden rounded-xl border border-border/60 bg-card/70 shadow-(--shadow-craft-inset) backdrop-blur"
  >
    <header
      class="flex items-center gap-1.5 border-b border-border/60 px-2.5 py-1.5 text-muted-foreground/80"
    >
      <Save size={11} />
      <span class="text-[10px] font-bold uppercase tracking-[0.15em]">
        Project
      </span>
    </header>
    <div class="space-y-1.5 px-2.5 py-2">
      <div class="flex items-center justify-between gap-2">
        <span class="text-muted-foreground">Save status</span>
        <span
          class="inline-flex items-center gap-1 rounded-full px-1.5 py-0.5 font-mono text-[10px] {saveStatus.tone ===
          'warning'
            ? 'bg-warning/15 text-warning ring-1 ring-warning/30'
            : saveStatus.tone === 'ok'
              ? 'bg-success/10 text-success ring-1 ring-success/30'
              : 'bg-muted text-muted-foreground ring-1 ring-border'}"
        >
          <span
            class="relative flex size-1.5"
            aria-hidden="true"
          >
            {#if saveStatus.tone === 'ok'}
              <span class="absolute inline-flex h-full w-full animate-ping rounded-full bg-success/50 opacity-70"></span>
            {/if}
            <span
              class="relative inline-flex size-1.5 rounded-full {saveStatus.tone === 'warning'
                ? 'bg-warning'
                : saveStatus.tone === 'ok'
                  ? 'bg-success'
                  : 'bg-muted-foreground'}"
            ></span>
          </span>
          {saveStatus.label}
        </span>
      </div>
      {#if store.lastSavedAt}
        <div class="flex items-center justify-between gap-2">
          <span class="text-muted-foreground">Last saved</span>
          <span class="font-mono tabular-nums text-foreground">
            {new Date(store.lastSavedAt).toLocaleString()}
          </span>
        </div>
      {/if}
      <div class="flex items-center justify-between gap-2">
        <span class="text-muted-foreground">Trim</span>
        <span class="inline-flex items-center gap-1 font-mono text-foreground">
          <Scissors size={10} class="text-muted-foreground" />
          {trimmed
            ? `${formatDuration(store.clipDuration)} kept`
            : "Full clip"}
        </span>
      </div>
      {#if trimmed}
        <div class="flex items-center justify-between gap-2 pl-3">
          <span class="text-muted-foreground">In / Out</span>
          <span class="font-mono tabular-nums text-foreground">
            {formatDuration(store.inPoint)} → {formatDuration(store.outPoint)}
          </span>
        </div>
      {/if}
      <div class="flex items-center justify-between gap-2">
        <span class="text-muted-foreground">Cursor track</span>
        <span class="font-mono text-foreground">
          {store.cursorPath ? "Captured" : "None"}
        </span>
      </div>
    </div>
  </section>

  <!-- Edit summary -->
  <section
    in:fly={{ y: 8, duration: 260, delay: 160, easing: cubicOut }}
    class="overflow-hidden rounded-xl border border-border/60 bg-card/70 shadow-(--shadow-craft-inset) backdrop-blur"
  >
    <header
      class="flex items-center gap-1.5 border-b border-border/60 px-2.5 py-1.5 text-muted-foreground/80"
    >
      <Pencil size={11} />
      <span class="text-[10px] font-bold uppercase tracking-[0.15em]">
        Edits
      </span>
    </header>
    <div class="space-y-1.5 px-2.5 py-2">
      <div class="flex items-center justify-between gap-2">
        <span class="text-muted-foreground">Focus regions</span>
        <span class="font-mono tabular-nums text-foreground">
          {store.zoomRegions.length}
        </span>
      </div>
      <div class="flex items-center justify-between gap-2">
        <span class="text-muted-foreground">Annotations</span>
        <span class="font-mono tabular-nums text-foreground">
          {totalAnnotations}
        </span>
      </div>
      {#if totalAnnotations > 0}
        <div
          class="grid grid-cols-5 gap-1 rounded-md border border-border/60 bg-background/40 p-1 shadow-(--shadow-craft-inset)"
        >
          {#each KIND_META as kind, i (kind.id)}
            {@const Icon = kind.icon}
            {@const count = annotationCounts[kind.id] ?? 0}
            <div
              in:scale={{ start: 0.85, duration: 220, delay: 180 + i * 30, easing: cubicOut }}
              class="flex flex-col items-center gap-0.5 rounded-sm px-1 py-1 transition-colors {count >
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
      <div class="flex items-center justify-between gap-2">
        <span class="flex items-center gap-1.5 text-muted-foreground">
          <MousePointer size={10} />
          Cursor overlay
        </span>
        <span class="font-mono text-foreground">
          {(store as { cursorEnabled?: boolean }).cursorEnabled ? "On" : "Off"}
        </span>
      </div>
      <div class="flex items-center justify-between gap-2">
        <span class="flex items-center gap-1.5 text-muted-foreground">
          {#if store.audioSettings?.muted}
            <VolumeX size={10} />
          {:else}
            <Volume2 size={10} />
          {/if}
          Audio
        </span>
        <span class="font-mono text-foreground">
          {store.audioSettings?.muted
            ? "Muted"
            : `${Math.round(store.audioSettings?.volume ?? 100)}%`}
        </span>
      </div>
      <div class="flex items-center justify-between gap-2">
        <span class="flex items-center gap-1.5 text-muted-foreground">
          <Stamp size={10} />
          Watermark
        </span>
        <span class="font-mono text-foreground">
          {store.watermarkSettings?.enabled ? "On" : "Off"}
        </span>
      </div>
    </div>
  </section>

  <!-- Files -->
  <section
    in:fly={{ y: 8, duration: 260, delay: 220, easing: cubicOut }}
    class="overflow-hidden rounded-xl border border-border/60 bg-card/70 shadow-(--shadow-craft-inset) backdrop-blur"
  >
    <header
      class="flex items-center gap-1.5 border-b border-border/60 px-2.5 py-1.5 text-muted-foreground/80"
    >
      <FileText size={11} />
      <span class="text-[10px] font-bold uppercase tracking-[0.15em]">
        Files
      </span>
    </header>
    <div class="space-y-2 px-2.5 py-2">
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
  </section>
</div>
