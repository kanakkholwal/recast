<script lang="ts" module>
import type { ScreenshotEditorState } from "../editor.svelte";
import type { Overlay } from "../types";

export interface OverlayLayerProps {
	editor: ScreenshotEditorState;
}

const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));
</script>

<script lang="ts">
  import { fontCss } from "../fonts";
  import { textShadowCss } from "../render";

  let { editor }: OverlayLayerProps = $props();

  let layerEl = $state<HTMLElement | null>(null);
  let editingId = $state<string | null>(null);

  // Pointer drag in stage-percent space, so overlays track the cursor at any
  // preview size and keep their position on export.
  function startDrag(e: PointerEvent, ov: Overlay) {
    if (editingId === ov.id || !layerEl) return;
    e.stopPropagation();
    editor.selectOverlay(ov.id);
    const rect = layerEl.getBoundingClientRect();
    const sx = e.clientX;
    const sy = e.clientY;
    const ox = ov.x;
    const oy = ov.y;
    const move = (ev: PointerEvent) => {
      editor.updateOverlay(ov.id, {
        x: clamp(ox + ((ev.clientX - sx) / rect.width) * 100, 0, 100),
        y: clamp(oy + ((ev.clientY - sy) / rect.height) * 100, 0, 100),
      });
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  // Resize an image overlay's width (height follows aspect) from its handle.
  function startResizeImage(e: PointerEvent, ov: Overlay) {
    if (ov.type !== "image" || !layerEl) return;
    e.stopPropagation();
    editor.selectOverlay(ov.id);
    const rect = layerEl.getBoundingClientRect();
    const sx = e.clientX;
    const os = ov.size;
    const move = (ev: PointerEvent) => {
      editor.updateOverlay(ov.id, {
        size: clamp(os + ((ev.clientX - sx) / rect.width) * 100 * 2, 5, 100),
      });
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  // Resize a shape/blur region from its bottom-right handle, in stage-percent space.
  function startResize(e: PointerEvent, ov: Overlay) {
    if ((ov.type !== "shape" && ov.type !== "blur") || !layerEl) return;
    e.stopPropagation();
    editor.selectOverlay(ov.id);
    const rect = layerEl.getBoundingClientRect();
    const sx = e.clientX;
    const sy = e.clientY;
    const ow = ov.w;
    const oh = ov.h;
    const move = (ev: PointerEvent) => {
      editor.updateOverlay(ov.id, {
        w: clamp(ow + ((ev.clientX - sx) / rect.width) * 100, 3, 100),
        h: clamp(oh + ((ev.clientY - sy) / rect.height) * 100, 3, 100),
      });
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  function onKey(e: KeyboardEvent, ov: Overlay) {
    if (e.key === "Enter" && ov.type === "text") {
      e.preventDefault();
      editingId = ov.id;
    } else if (e.key === "Delete" || e.key === "Backspace") {
      e.preventDefault();
      editor.removeOverlay(ov.id);
    } else if (e.key.startsWith("Arrow")) {
      e.preventDefault();
      const s = e.shiftKey ? 5 : 1;
      editor.updateOverlay(ov.id, {
        x: clamp(ov.x + (e.key === "ArrowLeft" ? -s : e.key === "ArrowRight" ? s : 0), 0, 100),
        y: clamp(ov.y + (e.key === "ArrowUp" ? -s : e.key === "ArrowDown" ? s : 0), 0, 100),
      });
    }
  }

  // Seed the contenteditable once and drop the caret at the end, without a
  // reactive write-back (which would fight the cursor).
  function initEditable(node: HTMLElement, text: string) {
    node.textContent = text;
    node.focus();
    const range = document.createRange();
    range.selectNodeContents(node);
    range.collapse(false);
    const sel = window.getSelection();
    sel?.removeAllRanges();
    sel?.addRange(range);
  }
</script>

<!-- z-2 lifts overlays (text/shapes/images/blur) above the framed screenshot
     (`.recast-shot-persp` is z-1), so a full-canvas light/shadow overlay and a
     blur region actually fall on the shot, not just the background. Guides sit
     higher (z-5/6). -->
<div bind:this={layerEl} class="pointer-events-none absolute inset-0 z-[2]">
  <!-- Clicking empty/image area clears the selection. A real button keeps this
       keyboard-accessible without a static-element interaction. -->
  <button
    type="button"
    class="pointer-events-auto absolute inset-0 cursor-default"
    aria-label="Clear selection"
    onclick={() => editor.selectOverlay(null)}
  ></button>

  {#each editor.overlays as ov (ov.id)}
    {#if !ov.isVisible}
      <!-- hidden overlay: not rendered, so it never reaches an export -->
    {:else if ov.type === "text"}
      <div
        class="pointer-events-auto absolute cursor-move touch-none select-none whitespace-nowrap outline-none"
        class:ring-2={editor.selectedId === ov.id}
        class:ring-primary={editor.selectedId === ov.id}
        class:ring-offset-1={editor.selectedId === ov.id}
        role="button"
        tabindex="0"
        style:left={`${ov.x}%`}
        style:top={`${ov.y}%`}
        style:opacity={ov.opacity}
        style:transform={`translate(-50%, -50%) rotate(${ov.rotation}deg)`}
        style:font-size={`${ov.fontSize}px`}
        style:font-family={fontCss(ov.fontFamily)}
        style:font-weight={ov.fontWeight}
        style:color={ov.color}
        style:text-align={ov.align}
        style:writing-mode={ov.orientation === "vertical" ? "vertical-rl" : "horizontal-tb"}
        style:text-shadow={textShadowCss(ov.shadow)}
        onpointerdown={(e) => startDrag(e, ov)}
        ondblclick={() => {
          editor.selectOverlay(ov.id);
          editingId = ov.id;
        }}
        onkeydown={(e) => onKey(e, ov)}
      >
        {#if editingId === ov.id}
          <div
            role="textbox"
            tabindex="0"
            aria-label="Edit text"
            contenteditable="true"
            class="outline-none"
            use:initEditable={ov.text}
            onblur={(e) => {
              editor.updateOverlay(ov.id, { text: e.currentTarget.textContent ?? "" });
              editingId = null;
            }}
            onkeydown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                e.currentTarget.blur();
              }
              e.stopPropagation();
            }}
          ></div>
        {:else}
          {ov.text}
        {/if}
      </div>
    {:else if ov.type === "shape"}
      <div
        class="pointer-events-auto absolute cursor-move touch-none outline-none"
        class:ring-2={editor.selectedId === ov.id}
        class:ring-primary={editor.selectedId === ov.id}
        class:ring-offset-1={editor.selectedId === ov.id}
        role="button"
        tabindex="0"
        style:left={`${ov.x}%`}
        style:top={`${ov.y}%`}
        style:width={`${ov.w}%`}
        style:height={`${ov.h}%`}
        style:opacity={ov.opacity}
        style:transform={`translate(-50%, -50%) rotate(${ov.rotation}deg)`}
        onpointerdown={(e) => startDrag(e, ov)}
        onkeydown={(e) => onKey(e, ov)}
      >
        <svg
          class="h-full w-full overflow-visible"
          viewBox="0 0 100 100"
          preserveAspectRatio="none"
          aria-hidden="true"
        >
          {#if ov.shape === "rectangle"}
            <rect
              x="2"
              y="2"
              width="96"
              height="96"
              rx="4"
              fill={ov.filled ? ov.fillColor : "none"}
              stroke={ov.strokeColor}
              stroke-width={ov.strokeWidth}
              vector-effect="non-scaling-stroke"
            />
          {:else if ov.shape === "ellipse"}
            <ellipse
              cx="50"
              cy="50"
              rx="48"
              ry="48"
              fill={ov.filled ? ov.fillColor : "none"}
              stroke={ov.strokeColor}
              stroke-width={ov.strokeWidth}
              vector-effect="non-scaling-stroke"
            />
          {:else if ov.shape === "line"}
            <line
              x1="6"
              y1="94"
              x2="94"
              y2="6"
              stroke={ov.strokeColor}
              stroke-width={ov.strokeWidth}
              vector-effect="non-scaling-stroke"
            />
          {:else if ov.shape === "arrow"}
            <defs>
              <marker
                id={`recast-arrow-${ov.id}`}
                markerWidth="10"
                markerHeight="10"
                refX="7"
                refY="3"
                orient="auto"
              >
                <path d="M0,0 L8,3 L0,6 Z" fill={ov.strokeColor} />
              </marker>
            </defs>
            <line
              x1="6"
              y1="6"
              x2="94"
              y2="94"
              stroke={ov.strokeColor}
              stroke-width={ov.strokeWidth}
              vector-effect="non-scaling-stroke"
              marker-end={`url(#recast-arrow-${ov.id})`}
            />
          {/if}
        </svg>

        {#if editor.selectedId === ov.id}
          <button
            type="button"
            class="border-primary bg-background absolute -right-1.5 -bottom-1.5 size-3 cursor-nwse-resize rounded-full border"
            aria-label="Resize"
            onpointerdown={(e) => startResize(e, ov)}
          ></button>
        {/if}
      </div>
    {:else if ov.type === "blur"}
      <div
        class="pointer-events-auto absolute cursor-move touch-none overflow-hidden rounded-sm outline-none"
        class:ring-2={editor.selectedId === ov.id}
        class:ring-primary={editor.selectedId === ov.id}
        class:ring-offset-1={editor.selectedId === ov.id}
        role="button"
        tabindex="0"
        style:left={`${ov.x}%`}
        style:top={`${ov.y}%`}
        style:width={`${ov.w}%`}
        style:height={`${ov.h}%`}
        style:opacity={ov.opacity}
        style:transform={`translate(-50%, -50%) rotate(${ov.rotation}deg)`}
        style={`backdrop-filter:blur(${ov.blurAmount}px);-webkit-backdrop-filter:blur(${ov.blurAmount}px);`}
        onpointerdown={(e) => startDrag(e, ov)}
        onkeydown={(e) => onKey(e, ov)}
      >
        {#if editor.selectedId === ov.id}
          <button
            type="button"
            class="border-primary bg-background absolute -right-1.5 -bottom-1.5 size-3 cursor-nwse-resize rounded-full border"
            aria-label="Resize"
            onpointerdown={(e) => startResize(e, ov)}
          ></button>
        {/if}
      </div>
    {:else if ov.type === "image"}
      <div
        class="pointer-events-auto absolute cursor-move touch-none outline-none"
        class:ring-2={editor.selectedId === ov.id}
        class:ring-primary={editor.selectedId === ov.id}
        class:ring-offset-1={editor.selectedId === ov.id}
        role="button"
        tabindex="0"
        style:left={`${ov.x}%`}
        style:top={`${ov.y}%`}
        style:width={`${ov.size}%`}
        style:opacity={ov.opacity}
        style:transform={`translate(-50%, -50%) rotate(${ov.rotation}deg)`}
        onpointerdown={(e) => startDrag(e, ov)}
        onkeydown={(e) => onKey(e, ov)}
      >
        <img
          src={ov.src}
          alt=""
          draggable="false"
          class="pointer-events-none block w-full select-none"
          style:filter={ov.blur > 0 ? `blur(${ov.blur}px)` : "none"}
          style:transform={`scale(${ov.flipX ? -1 : 1}, ${ov.flipY ? -1 : 1})`}
        />
        {#if editor.selectedId === ov.id}
          <button
            type="button"
            class="border-primary bg-background absolute -right-1.5 -bottom-1.5 size-3 cursor-nwse-resize rounded-full border"
            aria-label="Resize"
            onpointerdown={(e) => startResizeImage(e, ov)}
          ></button>
        {/if}
      </div>
    {/if}
  {/each}
</div>
