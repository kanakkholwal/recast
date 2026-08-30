<script lang="ts">
import { Crop } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { emit } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { cubicOut } from "svelte/easing";
import { scale } from "svelte/transition";
import { captureRegionShot } from "$lib/ipc";
import { notifyNow } from "$lib/notify";
import {
	clampToolbar,
	confirmLabel,
	hintLabel,
	overlayMode,
	rectFromPoints,
	savedMessage,
	TOOLBAR_W,
	toRegionPayload,
} from "./select-area.logic";

// Same drag, two endings: hand the area to the recorder, or capture it now.
const mode = overlayMode(window.location.search);
let capturing = $state(false);
let failure = $state<string | null>(null);

// Read on confirm, not on mount: the window is positioned after it is built.
async function virtualOrigin(): Promise<{ x: number; y: number }> {
	const pos = await getCurrentWindow().outerPosition();
	const scale = window.devicePixelRatio || 1;
	return { x: Math.round(pos.x / scale), y: Math.round(pos.y / scale) };
}

let dragging = $state(false);
let startX = $state(0);
let startY = $state(0);
let curX = $state(0);
let curY = $state(0);

// Last drawn rect (frozen after pointerup so the user can confirm).
let rect = $state<{ x: number; y: number; w: number; h: number } | null>(null);

function onPointerDown(e: PointerEvent) {
	dragging = true;
	rect = null;
	startX = e.clientX;
	startY = e.clientY;
	curX = e.clientX;
	curY = e.clientY;
	(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
}

function onPointerMove(e: PointerEvent) {
	if (!dragging) return;
	curX = e.clientX;
	curY = e.clientY;
}

function onPointerUp(e: PointerEvent) {
	if (!dragging) return;
	dragging = false;
	(e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
	const next = rectFromPoints(startX, startY, curX, curY);
	if (next.w < 8 || next.h < 8) {
		rect = null;
		return;
	}
	rect = next;
}

async function confirm() {
	if (!rect || capturing) return;
	let origin: { x: number; y: number };
	try {
		origin = await virtualOrigin();
	} catch (e) {
		failure = e instanceof Error ? e.message : String(e);
		return;
	}
	const payload = toRegionPayload(rect, origin, window.devicePixelRatio || 1);
	if (mode === "record") {
		emit("region-selected", payload);
		getCurrentWindow().close();
		return;
	}
	// The overlay captures itself: a shortcut can fire with no window alive to listen.
	capturing = true;
	failure = null;
	try {
		const shot = await captureRegionShot(payload);
		await emit("screenshot-captured", shot);
		// Awaited before the close, or the notification races this window's teardown.
		await notifyNow(...savedMessage(shot));
		getCurrentWindow().close();
	} catch (e) {
		capturing = false;
		failure = e instanceof Error ? e.message : String(e);
	}
}

function reset() {
	rect = null;
}

function cancel() {
	getCurrentWindow().close();
}

function onKey(e: KeyboardEvent) {
	if (capturing) return;
	if (e.key === "Escape") {
		e.preventDefault();
		// Esc always exits. Users expect the window to close. Use the
		// explicit "Redraw" button to clear a selection without exiting.
		cancel();
	} else if (e.key === "Enter" && rect) {
		e.preventDefault();
		confirm();
	}
}

// Live derived rect for display while dragging.
const liveRect = $derived(dragging ? rectFromPoints(startX, startY, curX, curY) : rect);

// Toolbar position, clamped to the viewport so it stays reachable when the
// selection lands near the bottom or right edge of the virtual desktop.
const toolbarPos = $derived.by(() =>
	rect ? clampToolbar(rect, window.innerWidth, window.innerHeight) : { left: 0, top: 0 },
);
</script>

<svelte:window onkeydown={onKey} />

<div
  role="presentation"
  class="absolute inset-0 cursor-crosshair select-none"
  style="background: rgba(0, 0, 0, 0.35);"
  onpointerdown={capturing ? undefined : onPointerDown}
  onpointermove={capturing ? undefined : onPointerMove}
  onpointerup={capturing ? undefined : onPointerUp}
>
  {#if liveRect && liveRect.w > 0 && liveRect.h > 0}
    <!-- Cut-out via box-shadow: rect is transparent, the dim layer is the outer
         box-shadow. pointer-events: none so clicks inside don't restart a drag.
         White marquee, not a theme token: this floats over arbitrary screen
         content on a dark scrim, so it must not follow the app theme. -->
    <div
      class="pointer-events-none absolute border border-white/90 ring-1 ring-black/40"
      style="left: {liveRect.x}px; top: {liveRect.y}px; width: {liveRect.w}px; height: {liveRect.h}px; background: transparent; box-shadow: 0 0 0 9999px rgba(0,0,0,0.45);"
    >
      <!-- Corner handles, decorative: the whole overlay redraws, not resizes. -->
      {#each [
        "left-0 top-0 -translate-x-1/2 -translate-y-1/2",
        "right-0 top-0 translate-x-1/2 -translate-y-1/2",
        "left-0 bottom-0 -translate-x-1/2 translate-y-1/2",
        "right-0 bottom-0 translate-x-1/2 translate-y-1/2",
      ] as pos (pos)}
        <span
          class="absolute size-2 rounded-full bg-white shadow-[0_0_0_1px_rgb(0_0_0/0.4)] {pos}"
        ></span>
      {/each}
    </div>

    <!-- Size badge -->
    <div
      class="pointer-events-none absolute rounded-md bg-background/95 px-1.5 py-0.5 font-mono text-[11px] font-semibold tabular-nums text-foreground shadow-craft-sm ring-1 ring-border/60 backdrop-blur"
      style="left: {liveRect.x}px; top: {Math.max(liveRect.y - 24, 0)}px;"
    >
      {Math.round(liveRect.w * (window.devicePixelRatio || 1))} × {Math.round(liveRect.h * (window.devicePixelRatio || 1))}
    </div>
  {/if}

  {#if !dragging && !rect}
    <div
      class="pointer-events-none absolute inset-0 flex items-center justify-center"
      in:scale={{ start: 0.96, duration: 200, easing: cubicOut }}
    >
      <div
        class="flex items-center gap-2 rounded-xl border border-border/60 bg-background/90 px-4 py-2.5 text-[12.5px] font-medium text-foreground shadow-craft-floating backdrop-blur-xl"
      >
        <Crop size={13} class="text-muted-foreground" />
        {hintLabel(mode)}
        <kbd
          class="rounded border border-border/60 bg-muted/60 px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground"
        >
          Esc
        </kbd>
      </div>
    </div>
  {/if}

  {#if failure}
    <!-- A failed capture keeps the overlay up with the selection intact, so the
         user can retry instead of losing the area they just drew. -->
    <div
      role="alert"
      class="pointer-events-none absolute inset-x-0 top-6 mx-auto w-fit rounded-xl border border-destructive/50 bg-background/95 px-4 py-2.5 text-[12.5px] font-medium text-foreground shadow-craft-floating backdrop-blur-xl"
    >
      Could not capture that area. {failure}
    </div>
  {/if}

  {#if rect && !dragging}
    <!-- Stop pointer events so clicks on the toolbar's padding don't bubble to
         the overlay and clear the rect. -->
    <div
      role="toolbar"
      aria-label="Confirm selected area"
      tabindex="0"
      class="absolute flex cursor-default items-center gap-1 rounded-xl border border-border/60 bg-background/95 p-1 shadow-craft-floating backdrop-blur-xl"
      style="left: {toolbarPos.left}px; top: {toolbarPos.top}px; min-width: {TOOLBAR_W}px;"
      in:scale={{ start: 0.96, duration: 160, easing: cubicOut }}
      onpointerdown={(e) => e.stopPropagation()}
      onpointerup={(e) => e.stopPropagation()}
    >
      <Button variant="ghost" size="xs" class="rounded-lg" onclick={reset} disabled={capturing}>
        Redraw
      </Button>
      <Button
        variant="ghost"
        size="xs"
        class="rounded-lg text-muted-foreground"
        onclick={cancel}
        disabled={capturing}
      >
        Cancel
      </Button>
      <Button
        variant="default"
        size="xs"
        class="rounded-lg"
        onclick={confirm}
        disabled={capturing}
        title="{confirmLabel(mode)} (Enter)"
      >
        {capturing ? "Capturing…" : confirmLabel(mode)}
      </Button>
    </div>
  {/if}
</div>

<style>
  /* The Tauri overlay window must be fully transparent so the screen
     beneath shows through the dim layer rendered in the page. */
  :global(html),
  :global(body) {
    background: transparent !important;
    margin: 0;
    padding: 0;
    overflow: hidden;
    height: 100vh;
  }
  /* Defeat the bg-background applied by the root +layout wrapper. */
  :global(body > div),
  :global(body > div > div) {
    background: transparent !important;
  }
</style>
