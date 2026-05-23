<script lang="ts">
  import type { RecordingEntry } from "$lib/ipc";
  import { openFileLocation } from "$lib/ipc";
  import { Clock, Download, FolderOpen, Video, X } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { RecastPlayer } from "@recast/player";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { cubicOut } from "svelte/easing";
  import { fade, scale } from "svelte/transition";

  let {
    entry,
    onclose,
  }: {
    entry: RecordingEntry;
    onclose: () => void;
  } = $props();

  // Tauri's asset:// URL — needed because the WebView can't read raw OS
  // paths. Recomputed if the parent swaps `entry` in place (rename flow).
  const src = $derived(convertFileSrc(entry.path));

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
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />

<div class="fixed inset-0 z-100 grid place-items-center p-4 sm:p-8">
  <button
    type="button"
    aria-label="Close player"
    onclick={onclose}
    class="absolute inset-0 cursor-default bg-background/80 backdrop-blur-sm"
    transition:fade={{ duration: 150 }}
  ></button>

  <div
    class="relative z-10 w-full max-w-3xl overflow-hidden rounded-2xl border border-border/60 bg-card shadow-2xl ring-1 ring-border/40"
    transition:scale={{ start: 0.96, duration: 240, easing: cubicOut }}
  >
    <header
      class="flex items-center gap-3 border-b border-border/50 px-4 py-3"
    >
      <Video class="size-4 shrink-0 text-primary" />
      <span
        class="min-w-0 flex-1 truncate text-sm font-semibold text-foreground"
        title={entry.filename}
      >
        {entry.filename}
      </span>
      <Button
        variant="ghost"
        size="icon-sm"
        onclick={onclose}
        aria-label="Close"
      >
        <X class="size-4" />
      </Button>
    </header>

    <RecastPlayer {src} title={entry.filename} autoplay />

    <footer
      class="flex flex-wrap items-center justify-between gap-x-4 gap-y-2 px-4 py-3 text-xs text-muted-foreground"
    >
      <div class="flex flex-wrap items-center gap-x-4 gap-y-1">
        <span class="flex items-center gap-1.5">
          <Download class="size-3.5" />
          {formatSize(entry.sizeBytes)}
        </span>
        <span class="flex items-center gap-1.5">
          <Clock class="size-3.5" />
          {formatDate(entry.created)}
        </span>
      </div>
      <Button
        variant="ghost"
        size="xs"
        class="h-7 gap-1.5 text-[11px]"
        onclick={() => openFileLocation(entry.path)}
      >
        <FolderOpen class="size-3.5" />
        Show in folder
      </Button>
    </footer>
  </div>
</div>
