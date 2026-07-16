<script lang="ts">
  import { formatDateTime, formatSize, isImageFile } from "$lib/format/files";
  import type { RecordingEntry } from "$lib/ipc";
  import { captionSidecarVtt, openFileLocation } from "$lib/ipc";
  import {
    Clock,
    Download,
    FolderOpen,
    Image as ImageIcon,
    Video,
    X,
  } from "@recast/icons";
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

  // Tauri's asset:// URL, needed because the WebView can't read raw OS
  // paths. Recomputed if the parent swaps `entry` in place (rename flow).
  const src = $derived(convertFileSrc(entry.path));

  // GIF (and other image) exports can't play in the video element, so they get
  // an <img> preview instead. GIFs loop on their own.
  const isImage = $derived(isImageFile(entry.filename));

  // Auto-load a caption sidecar written next to the export (foo.vtt / foo.srt).
  // The Rust side returns WebVTT (converting .srt); we hand it to the player as
  // a blob-URL <track>, so a previewed file shows its captions with no project.
  let captionSrc = $state<string | null>(null);
  $effect(() => {
    if (isImage) {
      captionSrc = null;
      return;
    }
    const path = entry.path;
    let url: string | null = null;
    let cancelled = false;
    captionSidecarVtt(path)
      .then((vtt) => {
        if (cancelled || !vtt) return;
        url = URL.createObjectURL(new Blob([vtt], { type: "text/vtt" }));
        captionSrc = url;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      if (url) URL.revokeObjectURL(url);
      captionSrc = null;
    };
  });
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
      {#if isImage}
        <ImageIcon class="size-4 shrink-0 text-primary" />
      {:else}
        <Video class="size-4 shrink-0 text-primary" />
      {/if}
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

    {#if isImage}
      <div
        class="flex max-h-[65vh] items-center justify-center overflow-hidden bg-muted/30"
      >
        <img
          {src}
          alt={entry.filename}
          draggable="false"
          class="max-h-[65vh] max-w-full object-contain"
        />
      </div>
    {:else}
      <!-- autohide={2.5}: the control bar fades out after ~2.5s of pointer
           inactivity and fades back on movement (see the .recast-control-bar
           transition in the player), matching a normal video player. -->
      <!-- preload="auto" (not "metadata"): exports are moov-at-end, and a
           metadata-only preload range-fetches the tail over the asset protocol and
           stalls in NETWORK_LOADING (black frame) in release. "auto" streams from
           byte 0. -->
      <RecastPlayer
        {src}
        title={entry.filename}
        preload="auto"
        autoplay
        autohide={2.5}
        tracks={captionSrc
          ? [
              {
                src: captionSrc,
                kind: "captions",
                label: "Captions",
                srclang: "en",
                default: true,
              },
            ]
          : []}
      />
    {/if}

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
          {formatDateTime(entry.created)}
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
