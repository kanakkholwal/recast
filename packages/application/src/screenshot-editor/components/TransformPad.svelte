<script lang="ts" module>
import type { ScreenshotEditorState } from "../editor.svelte";

export interface TransformPadProps {
	editor: ScreenshotEditorState;
}

const MAX = 30; // deg of tilt at the pad's edge
const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));
// 3x3 snap targets: [rotateX, rotateY] in deg (top row tilts up/back).
const SNAP: { x: number; y: number; label: string }[] = [
	{ x: MAX, y: -MAX, label: "Top left" },
	{ x: MAX, y: 0, label: "Top" },
	{ x: MAX, y: MAX, label: "Top right" },
	{ x: 0, y: -MAX, label: "Left" },
	{ x: 0, y: 0, label: "Center" },
	{ x: 0, y: MAX, label: "Right" },
	{ x: -MAX, y: -MAX, label: "Bottom left" },
	{ x: -MAX, y: 0, label: "Bottom" },
	{ x: -MAX, y: MAX, label: "Bottom right" },
];
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";

  let { editor }: TransformPadProps = $props();
  let padEl = $state<HTMLElement | null>(null);

  // Derived from the current tilt so the pad reflects slider and preset edits: rotateY drives x, rotateX drives y inverted.
  const dotX = $derived(clamp((editor.transform.rotateY / MAX + 1) / 2, 0, 1));
  const dotY = $derived(clamp((1 - editor.transform.rotateX / MAX) / 2, 0, 1));

  function setFromPoint(clientX: number, clientY: number) {
    if (!padEl) return;
    const r = padEl.getBoundingClientRect();
    const px = clamp((clientX - r.left) / r.width, 0, 1);
    const py = clamp((clientY - r.top) / r.height, 0, 1);
    editor.patchTransform({
      rotateY: Math.round((px * 2 - 1) * MAX),
      rotateX: Math.round((1 - py * 2) * MAX),
    });
  }

  function startDrag(e: PointerEvent) {
    e.preventDefault();
    padEl?.setPointerCapture(e.pointerId);
    editor.tiltDragging = true;
    setFromPoint(e.clientX, e.clientY);
    const move = (ev: PointerEvent) => setFromPoint(ev.clientX, ev.clientY);
    const up = () => {
      editor.tiltDragging = false;
      padEl?.releasePointerCapture(e.pointerId);
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }
</script>

<PanelSection variant="panel" title="Tilt" collapsible defaultOpen>
  <div class="flex gap-3">
    <!-- Drag pad with a snap grid overlay. -->
    <div
      bind:this={padEl}
      class="border-border bg-muted relative aspect-square w-28 shrink-0 cursor-crosshair touch-none rounded-lg border"
      role="application"
      aria-label="Drag to tilt"
      onpointerdown={startDrag}
    >
      <div class="pointer-events-none absolute inset-0 grid grid-cols-3 grid-rows-3">
        {#each SNAP as _cell, i (i)}
          <span class="border-border border"></span>
        {/each}
      </div>
      <div class="absolute inset-0 grid grid-cols-3 grid-rows-3">
        {#each SNAP as cell (cell.label)}
          <button
            type="button"
            class="hover:bg-accent rounded-sm transition-colors"
            aria-label={cell.label}
            onclick={() => editor.patchTransform({ rotateX: cell.x, rotateY: cell.y })}
          ></button>
        {/each}
      </div>
      <span
        class="border-primary bg-background pointer-events-none absolute size-3 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 shadow"
        style:left={`${dotX * 100}%`}
        style:top={`${dotY * 100}%`}
      ></span>
    </div>

    <!-- The pad owns X/Y tilt; Scale and Rotate live in the 3D perspective
         section below, so there is one home per value (no duplicate controls). -->
    <p class="text-muted-foreground min-w-0 flex-1 self-center text-xs">
      Drag to tilt. Scale and rotation are in 3D perspective below.
    </p>
  </div>
</PanelSection>
