<script lang="ts">
  import type { RegionRect } from "$lib/ipc";
  import { Button } from "@recast/ui/button";
  import { emit } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";

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
      // Esc always exits — users expect the window to close. Use the
      // explicit "Redraw" button to clear a selection without exiting.
      cancel();
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

  // Toolbar position, clamped to the viewport so it stays reachable when the
  // selection lands near the bottom or right edge of the virtual desktop.
  const TOOLBAR_W = 240;
  const TOOLBAR_H = 36;
  const toolbarPos = $derived.by(() => {
    if (!rect) return { left: 0, top: 0 };
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    const desiredTop = rect.y + rect.h + 6;
    const top =
      desiredTop + TOOLBAR_H + 8 > vh
        ? Math.max(8, rect.y - TOOLBAR_H - 6)
        : desiredTop;
    const left = Math.max(8, Math.min(rect.x, vw - TOOLBAR_W - 8));
    return { left, top };
  });
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
         layer is painted by an outer box-shadow on this element.
         pointer-events: none so clicks inside the rect don't restart a drag. -->
    <div
      class="pointer-events-none absolute border border-primary/90 ring-1 ring-primary/40"
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
    <!-- Confirm toolbar — stop pointer events on the wrapper itself so clicks
         on its padding don't bubble to the overlay (which would clear the
         rect mid-click). Position is clamped into the viewport. -->
    <div
      role="toolbar"
      aria-label="Confirm selected area"
      tabindex="0"
      class="absolute flex items-center gap-1.5 bg-background/95 backdrop-blur border border-border-subtle rounded-md p-1 shadow-craft-floating cursor-default"
      style="left: {toolbarPos.left}px; top: {toolbarPos.top}px; min-width: {TOOLBAR_W}px;"
      onpointerdown={(e) => e.stopPropagation()}
      onpointerup={(e) => e.stopPropagation()}
    >
      <Button variant="ghost" size="xs" onclick={reset}>Redraw</Button>
      <Button variant="ghost" size="xs" onclick={cancel}>Cancel</Button>
      <Button variant="default" size="xs" onclick={confirm}>Use area</Button>
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
