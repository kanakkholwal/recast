<script lang="ts">
  import { Button } from "@recast/ui/button";
  import { emit } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import type { RegionRect } from "$lib/ipc";

  // The overlay window is created at virtual desktop origin, sized to span all
  // monitors. Pointer coords from the window therefore equal virtual-desktop
  // pixel coords, which is what the Rust resolver expects.
  let originX = $state(0);
  let originY = $state(0);

  let dragging = $state(false);
  let startX = $state(0);
  let startY = $state(0);
  let curX = $state(0);
  let curY = $state(0);

  // Last drawn rect (frozen after pointerup so the user can confirm).
  let rect = $state<{ x: number; y: number; w: number; h: number } | null>(
    null,
  );

  onMount(() => {
    // Read window position so we can convert local pointer to global coords.
    const win = getCurrentWindow();
    win
      .outerPosition()
      .then((pos) => {
        const scale = window.devicePixelRatio || 1;
        originX = Math.round(pos.x / scale);
        originY = Math.round(pos.y / scale);
      })
      .catch(() => {});
  });

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
    const x = Math.min(startX, curX);
    const y = Math.min(startY, curY);
    const w = Math.abs(curX - startX);
    const h = Math.abs(curY - startY);
    if (w < 8 || h < 8) {
      rect = null;
      return;
    }
    rect = { x, y, w, h };
  }

  function confirm() {
    if (!rect) return;
    const dpr = window.devicePixelRatio || 1;
    const payload: RegionRect & { label: string } = {
      x: Math.round((rect.x + originX) * dpr),
      y: Math.round((rect.y + originY) * dpr),
      width: Math.round(rect.w * dpr),
      height: Math.round(rect.h * dpr),
      label: `Area ${Math.round(rect.w * dpr)}×${Math.round(rect.h * dpr)}`,
    };
    emit("region-selected", payload);
    getCurrentWindow().close();
  }

  function reset() {
    rect = null;
  }

  function cancel() {
    getCurrentWindow().close();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      if (rect) reset();
      else cancel();
    } else if (e.key === "Enter" && rect) {
      e.preventDefault();
      confirm();
    }
  }

  // Live derived rect for display while dragging.
  const liveRect = $derived(
    dragging
      ? {
          x: Math.min(startX, curX),
          y: Math.min(startY, curY),
          w: Math.abs(curX - startX),
          h: Math.abs(curY - startY),
        }
      : rect,
  );
</script>

<svelte:window onkeydown={onKey} />

<!-- Fullscreen transparent overlay; pointer events drive the selection. -->
<div
  role="presentation"
  class="absolute inset-0 cursor-crosshair select-none"
  style="background: rgba(0, 0, 0, 0.35);"
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
>
  {#if liveRect && liveRect.w > 0 && liveRect.h > 0}
    <!-- Cut-out via box-shadow trick: rect itself is transparent, the dim
         layer is painted by an outer box-shadow on this element. -->
    <div
      class="absolute border border-primary/90 ring-1 ring-primary/40"
      style="left: {liveRect.x}px; top: {liveRect.y}px; width: {liveRect.w}px; height: {liveRect.h}px; background: transparent; box-shadow: 0 0 0 9999px rgba(0,0,0,0.45);"
    ></div>

    <!-- Size badge -->
    <div
      class="absolute font-mono text-[11px] font-semibold tabular-nums text-primary-foreground bg-primary px-1.5 py-0.5 rounded-sm shadow-craft-sm pointer-events-none"
      style="left: {liveRect.x}px; top: {Math.max(liveRect.y - 22, 0)}px;"
    >
      {Math.round(liveRect.w * (window.devicePixelRatio || 1))} × {Math.round(liveRect.h * (window.devicePixelRatio || 1))}
    </div>
  {/if}

  {#if !dragging && !rect}
    <div
      class="absolute inset-0 flex items-center justify-center pointer-events-none"
    >
      <div
        class="rounded-md border border-border-subtle bg-background/85 backdrop-blur px-4 py-2 shadow-craft-floating text-[12px] font-medium text-foreground"
      >
        Drag to select an area · <span class="text-muted-foreground"
          >Esc to cancel</span
        >
      </div>
    </div>
  {/if}

  {#if rect && !dragging}
    <!-- Confirm toolbar pinned near rect -->
    <div
      class="absolute flex items-center gap-1.5 bg-background/95 backdrop-blur border border-border-subtle rounded-md p-1 shadow-craft-floating pointer-events-auto"
      style="left: {rect.x}px; top: {rect.y + rect.h + 6}px;"
    >
      <Button
        variant="ghost"
        size="xs"
        onclick={reset}
        onpointerdown={(e) => e.stopPropagation()}
      >
        Redraw
      </Button>
      <Button
        variant="ghost"
        size="xs"
        onclick={cancel}
        onpointerdown={(e) => e.stopPropagation()}
      >
        Cancel
      </Button>
      <Button
        variant="default"
        size="xs"
        onclick={confirm}
        onpointerdown={(e) => e.stopPropagation()}
      >
        Use area
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
