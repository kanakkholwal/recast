<script lang="ts">
  import { goto } from "$app/navigation";
  import { Button } from "$components/ui/button";
  import { generateThumbnails, openFileLocation, type RecordingEntry } from "$lib/ipc";
  import { FolderOpen, Pencil, Play } from "@lucide/svelte";
  import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
  import type { Snippet } from "svelte";
  import { onMount } from "svelte";

  interface Props {
    entries: RecordingEntry[];
    isLoading: boolean;
    emptyTitle?: string;
    emptyDescription?: string;
    emptyIcon?: Snippet;
    skeleton?: Snippet;
    showEditButton?: boolean;
  }

  let {
    entries,
    isLoading,
    emptyTitle = "No files yet",
    emptyDescription = "",
    emptyIcon,
    skeleton,
    showEditButton = false,
  }: Props = $props();
  let editorWindow = $state<"navigate" | "new-window">("navigate");

  type ThumbnailMap = Record<string, string>;
  let thumbnails = $state<ThumbnailMap>({});
  let thumbnailPass = 0;

  $effect(() => {
    if (entries.length > 0) loadThumbnails(entries);
  });
  onMount(() => {
   const storedEditorBehavior = (localStorage.getItem("recast-editor-window")) as "navigate" | "new-window" | null;
    if (storedEditorBehavior !== null) {
      editorWindow = storedEditorBehavior
    }
  });

  async function loadThumbnails(items: RecordingEntry[]) {
    const pass = ++thumbnailPass;
    const settled = await Promise.allSettled(
      items.map(async (item) => {
        const frames = await generateThumbnails(item.path, 1);
        return [item.path, frames[0] ?? ""] as const;
      }),
    );
    if (pass !== thumbnailPass) return;
    const next: ThumbnailMap = {};
    for (const result of settled) {
      if (result.status === "fulfilled" && result.value[1]) {
        next[result.value[0]] = result.value[1];
      }
    }
    thumbnails = next;
  }

  function formatSize(bytes: number) {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / 1048576).toFixed(1)} MB`;
  }

  function formatDate(unix: number) {
    return new Date(unix * 1000).toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  function encodeEditorPath(path: string) {
    return encodeURIComponent(btoa(encodeURIComponent(path)));
  }

  async function navigateToEditor(path: string, filename: string) {
    const route = `/editor/${encodeEditorPath(path)}`;
    if (editorWindow === "new-window") {
      const label = `editor-${encodeEditorPath(path).replace(/[^a-zA-Z0-9]/g, "").slice(0, 48)}`;
      const existing = await WebviewWindow.getByLabel(label);
      if (existing) {
        await existing.setFocus();
        return;
      }
      new WebviewWindow(label, {
        url: route,
        title: `Editor - ${filename}`,
        width: 1440,
        height: 960,
        center: true,
        decorations: false,
      });
    } else {
      goto(route);
    }
  }
</script>

{#if isLoading && skeleton}
  {@render skeleton()}
{:else if entries.length === 0}
  <div class="flex flex-col items-center justify-center gap-4 rounded-2xl border border-dashed border-border bg-card/50 py-20">
    {#if emptyIcon}
      {@render emptyIcon()}
    {/if}
    <div class="text-center">
      <h3 class="text-base font-semibold text-foreground">{emptyTitle}</h3>
      {#if emptyDescription}
        <p class="mt-1 text-sm text-muted-foreground">{emptyDescription}</p>
      {/if}
    </div>
  </div>
{:else}
  <div class="grid grid-cols-1 gap-4 lg:grid-cols-2 2xl:grid-cols-3">
    {#each entries as item, i}
      <div
        class="group overflow-hidden rounded-xl border border-border bg-card transition-shadow hover:shadow-md"
        style="animation-delay: {i * 40}ms"
      >
        <div class="relative aspect-video w-full overflow-hidden bg-muted">
          {#if thumbnails[item.path]}
            <img
              src={thumbnails[item.path]}
              alt={item.filename}
              class="h-full w-full object-cover transition-transform duration-500 group-hover:scale-[1.02]"
              draggable="false"
            />
          {:else}
            <div class="flex h-full w-full items-center justify-center text-muted-foreground/30">
              <Play size={32} />
            </div>
          {/if}
        </div>

        <div class="flex items-center justify-between gap-3 p-3">
          <div class="min-w-0 flex-1">
            <p class="truncate text-sm font-medium text-foreground">{item.filename}</p>
            <div class="mt-1 flex items-center gap-2 text-xs text-muted-foreground">
              <span>{formatSize(item.sizeBytes)}</span>
              <span>&middot;</span>
              <span>{formatDate(item.created)}</span>
            </div>
          </div>

          <div class="flex items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100">
            {#if showEditButton}
              <Button
                variant="ghost"
                size="icon-sm"
                onclick={() => navigateToEditor(item.path, item.filename)}
                title="Edit"
              >
                <Pencil size={14} />
              </Button>
            {/if}
            <Button
              variant="ghost"
              size="icon-sm"
              onclick={() => openFileLocation(item.path)}
              title="Show in folder"
            >
              <FolderOpen size={14} />
            </Button>
          </div>
        </div>
      </div>
    {/each}
  </div>
{/if}
