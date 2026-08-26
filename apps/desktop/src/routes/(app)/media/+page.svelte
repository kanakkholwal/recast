<script lang="ts">
import {
	AssetCard,
	LibraryError,
	LibrarySearch,
	LibrarySkeletonGrid,
	LibrarySortSelect,
	LibraryViewToggle,
} from "$components/library";
import LibraryEmpty from "$components/library/LibraryEmpty.svelte";
import StudioPage from "$components/layout/StudioPage.svelte";
import { PlayerDialog } from "$components/recast";
import { openFileLocation, type RecordingEntry } from "$lib/ipc";
import { cardShellClass, listClass } from "$lib/library/card-styles";
import { openInEditor as openEditorWindow } from "$lib/library/editor-window";
import { formatSize, getExtension } from "@recast/editor/lib/format/files";
import { motionDuration } from "@recast/editor/lib/motion.svelte";
import {
	FileText,
	Film,
	FolderOpen,
	Image as ImageIcon,
	Music2,
	Pencil,
	Play,
	RefreshCw,
	type IconComponent,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import { safeStorage } from "@recast/ui/persisted-state";
import { Segmented } from "@recast/ui/segmented";
import { onMount } from "svelte";
import { fade } from "svelte/transition";
import { MEDIA_TABS, type MediaItem, type MediaKind, type MediaTab } from "./media.logic";
import { createMediaState } from "./media.svelte";

const media = createMediaState();
let editorWindow = $state<"navigate" | "new-window">("navigate");
let playTarget = $state<RecordingEntry | null>(null);

onMount(() => {
	media.refresh();
	media.restoreView();
	editorWindow = safeStorage.get<"navigate" | "new-window">("recast-editor-window", editorWindow);
});

const KIND_ICON: Record<MediaKind, IconComponent> = {
	video: Film,
	audio: Music2,
	image: ImageIcon,
	other: FileText,
};

function openItem(m: MediaItem) {
	if (m.source === "recording") openEditorWindow(m.entry, editorWindow);
	else if (m.kind === "video") playTarget = m.entry;
	else openFileLocation(m.entry.path);
}

const openLabel = (m: MediaItem) =>
	m.source === "recording" ? "Open in editor" : m.kind === "video" ? "Play" : "Open";
</script>

<StudioPage title="Media" subtitle={media.status === "ready" ? `${formatSize(media.totalSize)} · all your media in one place` : "All your media in one place"}>
  {#snippet filters()}
    <Segmented
      options={MEDIA_TABS}
      value={media.tab}
      onValueChange={(v) => (media.tab = v as MediaTab)}
      fill={false}
      aria-label="Filter media by type"
    />
    <div class="min-w-[160px] flex-1">
      <LibrarySearch bind:value={media.query} noun="media" />
    </div>
    <LibrarySortSelect bind:value={media.sort} noun="media" />
    <LibraryViewToggle bind:value={media.view} />
    <Button
      variant="ghost"
      size="icon-sm"
      onclick={media.refresh}
      disabled={media.isLoading}
      aria-label="Refresh media"
      title="Refresh"
    >
      <RefreshCw size={12} class={media.isLoading ? "motion-safe:animate-spin" : ""} />
    </Button>
  {/snippet}

  {#if media.status === "loading"}
    <LibrarySkeletonGrid view={media.view} />
  {:else if media.status === "error"}
    <LibraryError title="Couldn't load your media" message={media.loadError ?? "Unknown error"} onRetry={media.refresh} />
  {:else if media.status === "empty" || media.status === "no-matches"}
    <LibraryEmpty
      icon={Film}
      title={media.query ? "No matches" : "No media yet"}
      description={media.query
        ? `Nothing matches "${media.query}".`
        : "Your recordings and exports show up here, browsable in one place."}
    />
  {:else}
    <div class={listClass(media.view)}>
      {#each media.displayed as m, i (m.entry.path)}
        {@const isSelected = media.isSelected(m.entry.path)}
        <div
          in:fade={{ duration: motionDuration(200), delay: motionDuration(Math.min(i * 25, 200)) }}
          title={m.entry.filename}
          class={cardShellClass(media.view, isSelected)}
        >
          <AssetCard
            entry={m.entry}
            thumbnail={media.thumbnails[m.entry.path]}
            view={media.view}
            selected={isSelected}
            placeholderIcon={KIND_ICON[m.kind]}
            typeLabel={getExtension(m.entry.filename)}
            onOpen={() => media.select(m.entry.path)}
          />
        </div>
      {/each}
    </div>
  {/if}

  {#snippet footer()}
    <div class="flex h-16 items-center gap-3 px-5">
      {#if media.selected}
        {@const m = media.selected!}
        {@const Icon = KIND_ICON[m.kind]}
        <div class="grid aspect-video h-10 shrink-0 place-items-center overflow-hidden rounded-md border border-border/40 bg-muted/40">
          {#if media.thumbnails[m.entry.path]}
            <img src={media.thumbnails[m.entry.path]} alt="" class="size-full object-cover" />
          {:else}
            <Icon class="size-4 text-muted-foreground/60" />
          {/if}
        </div>
        <div class="min-w-0 flex-1">
          <div class="truncate text-[12.5px] font-semibold text-foreground">{m.entry.filename}</div>
          <div class="truncate text-[10.5px] capitalize text-muted-foreground/80">
            {m.kind} · {formatSize(m.entry.sizeBytes)}
          </div>
        </div>
        <Button
          variant="ghost"
          size="sm"
          class="gap-1.5 text-muted-foreground hover:text-foreground"
          onclick={() => openFileLocation(m.entry.path)}
        >
          <FolderOpen class="size-3.5" /> Show in folder
        </Button>
        <Button size="sm" class="gap-1.5" onclick={() => openItem(m)}>
          {#if m.source === "recording"}<Pencil class="size-3.5" />{:else}<Play class="size-3.5" />{/if}
          {openLabel(m)}
        </Button>
      {:else}
        <div class="grid aspect-video h-10 shrink-0 place-items-center rounded-md border border-dashed border-border/50 text-muted-foreground/40">
          <Play class="size-4" />
        </div>
        <div class="min-w-0 flex-1">
          <div class="text-[12.5px] font-medium text-muted-foreground">Nothing selected</div>
          <div class="text-[10.5px] text-muted-foreground/70">Select a media item to preview and open it.</div>
        </div>
      {/if}
    </div>
  {/snippet}
</StudioPage>

{#if playTarget}
  <PlayerDialog entry={playTarget} onclose={() => (playTarget = null)} />
{/if}
