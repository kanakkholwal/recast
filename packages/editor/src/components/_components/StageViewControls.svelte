<script lang="ts">
import { Camera, LoaderCircle, Maximize2, Minimize2 } from "@recast/icons";
import { toast } from "@recast/ui/sonner";
import * as Tooltip from "@recast/ui/tooltip";
import { cn } from "@recast/ui/utils";
import type { EditorStore } from "../../stores/editor-store.svelte";
import { BAR_BTN, BAR_BTN_DISABLED } from "./player-bar.styles";

let {
	store,
	captureFrame = undefined,
	fullscreenTargetEl = null,
}: {
	store: EditorStore;
	captureFrame?: (() => Promise<Blob | null>) | undefined;
	fullscreenTargetEl?: HTMLElement | null;
} = $props();

let capturing = $state(false);
let isFullscreen = $state(false);

$effect(() => {
	const handler = () => {
		isFullscreen = Boolean(document.fullscreenElement);
	};
	document.addEventListener("fullscreenchange", handler);
	return () => document.removeEventListener("fullscreenchange", handler);
});

async function copyFrameToClipboard() {
	if (capturing || !captureFrame) return;
	capturing = true;
	if (store.isPlaying) store.isPlaying = false;
	try {
		const blob = await captureFrame();
		if (!blob) {
			toast.error("Couldn't capture frame. Preview isn't ready yet.");
			return;
		}
		await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
		toast.success("Frame copied to clipboard.");
	} catch (err) {
		toast.error(`Couldn't copy frame: ${(err as Error)?.message ?? String(err)}`);
	} finally {
		capturing = false;
	}
}

async function toggleFullscreen() {
	if (document.fullscreenElement) {
		await document.exitFullscreen();
		return;
	}
	if (fullscreenTargetEl) await fullscreenTargetEl.requestFullscreen();
}
</script>

<div
  class="flex items-center gap-0.5 rounded-md bg-card p-0.5 shadow-craft-md ring-1 ring-inset ring-border/40"
>
  <Tooltip.Root>
    <Tooltip.Trigger>
      {#snippet child({ props })}
        <span {...props as Record<string, unknown>} class="inline-flex">
          <button
            type="button"
            onclick={copyFrameToClipboard}
            disabled={!captureFrame || capturing}
            aria-label="Copy current frame to clipboard"
            class={cn(BAR_BTN, BAR_BTN_DISABLED)}
          >
            {#if capturing}<LoaderCircle size={13} class="animate-spin" />{:else}<Camera
                size={13}
              />{/if}
          </button>
        </span>
      {/snippet}
    </Tooltip.Trigger>
    <Tooltip.Content>
      {capturing ? "Copying frame…" : !captureFrame ? "Preview isn't ready yet" : "Copy frame"}
    </Tooltip.Content>
  </Tooltip.Root>

  <Tooltip.Root>
    <Tooltip.Trigger>
      {#snippet child({ props })}
        <span {...props as Record<string, unknown>} class="inline-flex">
          <button
            type="button"
            onclick={toggleFullscreen}
            disabled={!fullscreenTargetEl}
            aria-label={isFullscreen ? "Exit fullscreen" : "Enter fullscreen"}
            class={cn(BAR_BTN, BAR_BTN_DISABLED)}
          >
            {#if isFullscreen}<Minimize2 size={13} />{:else}<Maximize2 size={13} />{/if}
          </button>
        </span>
      {/snippet}
    </Tooltip.Trigger>
    <Tooltip.Content>{isFullscreen ? "Exit fullscreen" : "Fullscreen"}</Tooltip.Content>
  </Tooltip.Root>
</div>
