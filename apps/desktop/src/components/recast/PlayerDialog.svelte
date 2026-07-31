<script lang="ts">
import { formatDateTime, formatSize, isImageFile } from "$lib/format/files";
import type { RecordingEntry } from "$lib/ipc";
import { captionSidecarVtt, openFileLocation } from "$lib/ipc";
import { FolderOpen, Image as ImageIcon, Video } from "@recast/icons";
import { Button } from "@recast/ui/button";
import DialogShell from "./DialogShell.svelte";
import { RecastPlayer } from "@recast/player";
import { convertFileSrc } from "@tauri-apps/api/core";

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

<DialogShell
  open={true}
  title={entry.filename}
  subtitle={`${formatSize(entry.sizeBytes)} · ${formatDateTime(entry.created)}`}
  icon={isImage ? ImageIcon : Video}
  widthClass="max-w-3xl"
  bodyClass="p-0!"
  onOpenChange={(v) => {
    if (!v) onclose();
  }}
>

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

  {#snippet footer()}
    <Button
      variant="ghost"
      size="xs"
      class="mr-auto gap-1.5"
      onclick={() => openFileLocation(entry.path)}
    >
      <FolderOpen class="size-3.5" />
      Show in folder
    </Button>
    <Button variant="ghost" size="xs" onclick={onclose}>Close</Button>
  {/snippet}
</DialogShell>
