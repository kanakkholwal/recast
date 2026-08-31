<script lang="ts">
import DialogShell from "@recast/editor/components/dialog/DialogShell.svelte";
import { formatDateTime, formatSize, isImageFile } from "@recast/editor/lib/format/files";
import { FolderOpen, Image as ImageIcon, Video } from "@recast/icons";
import { RecastPlayer } from "@recast/player";
import { Button } from "@recast/ui/button";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { RecordingEntry } from "$lib/ipc";
import { captionSidecarVtt, openFileLocation } from "$lib/ipc";

let {
	entry,
	onclose,
}: {
	entry: RecordingEntry;
	onclose: () => void;
} = $props();

// The WebView can't read raw OS paths; recomputed if the parent swaps `entry` in place, as the rename flow does.
const src = $derived(convertFileSrc(entry.path));

// Image exports can't play in the video element, so they get an <img> preview; GIFs loop on their own.
const isImage = $derived(isImageFile(entry.filename));

// Rust returns WebVTT (converting .srt), handed to the player as a blob-URL track, so a previewed file shows captions with no project.
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
		.catch(() => undefined);
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
  widthClass="sm:max-w-3xl"
  bodyClass="p-0! max-h-none"
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
      size="sm"
      class="mr-auto gap-1.5"
      onclick={() => openFileLocation(entry.path)}
    >
      <FolderOpen class="size-3.5" />
      Show in folder
    </Button>
    <Button variant="ghost" size="sm" onclick={onclose}>Close</Button>
  {/snippet}
</DialogShell>
