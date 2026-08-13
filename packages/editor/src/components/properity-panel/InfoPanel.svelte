<script lang="ts">
import { formatBytes as formatBytesBase } from "../../lib/format/bytes";
import { clock } from "../../lib/format/time";
import { getEditorServices } from "../../lib/editor/services";
import type { AnnotationKindName, EditorStore, PanelTab } from "../../stores/editor-store.svelte";
import type { IconComponent } from "@recast/icons";
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
	Target,
	Type as TypeIcon,
	Volume2,
	VolumeX,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import { toast } from "@recast/ui/sonner";
import * as Tooltip from "@recast/ui/tooltip";
import { onDestroy, onMount } from "svelte";
import { countByKind, formatRelative } from "./info-panel.logic";
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

// InfoPanel shows "--" for missing sizes, not the shared "0 B".
const formatBytes = (bytes: number | undefined): string =>
	formatBytesBase(bytes, { zeroLabel: "--" });

// Every kind always rendered (with 0) so the row doesn't shift as shapes change.
const KIND_META: Array<{
	id: AnnotationKindName;
	label: string;
	icon: IconComponent;
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
	[formatDuration(store.metadata?.duration), formatResolution(), formatFps()]
		.filter((s) => s && s !== "Unknown" && s !== "--" && s !== "--:--")
		.join(" · ") || "No metadata",
);

const cursorOn = $derived(store.cursorSettings.enabled);
const muted = $derived(store.audioSettings?.muted ?? false);
const audioValue = $derived(muted ? "Muted" : `${Math.round(store.audioSettings?.volume ?? 100)}%`);

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

const shell = getEditorServices().shell;

async function revealInFolder(path: string) {
	if (!path || !shell) return;
	try {
		await shell.openFileLocation(path);
	} catch (err) {
		const msg = typeof err === "string" ? err : String(err);
		toast.error(`Could not open folder: ${msg}`);
	}
}
</script>

<!-- Read-only stat row: icon + label on the left, mono value on the right. -->
{#snippet stat(Icon: IconComponent, label: string, value: string)}
  <div class="flex items-center justify-between gap-2 px-1.5 py-1">
    <span class="flex items-center gap-1.5 text-muted-foreground">
      <Icon size={11} />
      {label}
    </span>
    <span class="font-mono tabular-nums text-foreground">{value}</span>
  </div>
{/snippet}

<!-- Actionable stat row: jumps to the related panel on click. -->
{#snippet navStat(Icon: IconComponent, label: string, value: string, tab: PanelTab)}
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
          <!-- `clock`, not `formatDuration`: an in-point of 0 is valid and must
               read "0:00", where formatDuration's missing-metadata guard shows "--:--". -->
          <span class="font-mono tabular-nums text-foreground">
            {clock(store.inPoint)} → {clock(store.outPoint)}
          </span>
        </div>
      {/if}
      <div class="flex items-center justify-between gap-2 px-1.5 py-1">
        <span class="text-muted-foreground">Save state</span>
        <span
          class="text-foreground"
          title={store.lastSavedAt
            ? new Date(store.lastSavedAt).toLocaleString()
            : undefined}
        >
          {saveStatus.label}
        </span>
      </div>
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
            <!-- Counts are read-only data, not selection or action, so they stay
                 neutral: a primary tint here is the decorative-accent drift. -->
            <div
              class="flex flex-col items-center gap-0.5 rounded-sm px-1 py-1 {count >
              0
                ? 'bg-foreground/5 text-foreground ring-1 ring-border/60'
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
    </div>
  </PanelSection>

  <PanelSection title="Files" flush>
    <div class="space-y-2">
      <div class="space-y-1">
        <div class="flex items-center justify-between gap-2">
          <span class="text-muted-foreground">Recording</span>
          <div class="flex items-center gap-0.5">
            <Tooltip.Root>
              <Tooltip.Trigger>
                {#snippet child({ props })}
                  <Button
                    {...props as Record<string, unknown>}
                    variant="ghost"
                    size="icon-sm"
                    onclick={() => copyToClipboard(store.videoPath, "Path")}
                    aria-label="Copy recording path"
                  >
                    <Copy size={11} />
                  </Button>
                {/snippet}
              </Tooltip.Trigger>
              <Tooltip.Content>Copy path</Tooltip.Content>
            </Tooltip.Root>
            {#if shell}
              <Tooltip.Root>
                <Tooltip.Trigger>
                  {#snippet child({ props })}
                    <Button
                      {...props as Record<string, unknown>}
                      variant="ghost"
                      size="icon-sm"
                      onclick={() => revealInFolder(store.videoPath)}
                      aria-label="Reveal in folder"
                    >
                      <FolderOpen size={11} />
                    </Button>
                  {/snippet}
                </Tooltip.Trigger>
                <Tooltip.Content>Reveal in folder</Tooltip.Content>
              </Tooltip.Root>
            {/if}
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
            <Tooltip.Root>
              <Tooltip.Trigger>
                {#snippet child({ props })}
                  <Button
                    {...props as Record<string, unknown>}
                    variant="ghost"
                    size="icon-sm"
                    onclick={() =>
                      store.cursorPath &&
                      copyToClipboard(store.cursorPath, "Cursor path")}
                    aria-label="Copy cursor track path"
                  >
                    <Copy size={11} />
                  </Button>
                {/snippet}
              </Tooltip.Trigger>
              <Tooltip.Content>Copy path</Tooltip.Content>
            </Tooltip.Root>
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
