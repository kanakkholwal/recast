<script lang="ts" module>
  import type { ScreenshotEditorState } from "../editor.svelte";
  import type { Overlay } from "../types";

  export interface LayerControlProps {
    editor: ScreenshotEditorState;
  }

  function layerLabel(o: Overlay): string {
    if (o.type === "text") return o.text.trim() || "Text";
    return o.shape.charAt(0).toUpperCase() + o.shape.slice(1);
  }
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { SliderControl } from "@recast/ui/slider-control";
  import { Button } from "@recast/ui/button";
  import { cn } from "@recast/ui/utils";
  import { ArrowDown, ArrowUp, Circle, Square, Trash2, Type } from "@recast/icons";
  import type { ShapeOverlay, TextOverlay } from "../types";

  let { editor }: LayerControlProps = $props();

  // Top layer first (paint order is bottom-up, so reverse for display).
  const layers = $derived([...editor.overlays].reverse());

  // Geometry for the selected layer. Colour/stroke/text live in their own
  // Design-tab sections; this covers what nothing else exposes.
  const selected = $derived(editor.selectedOverlay);
  const selText = $derived(selected?.type === "text" ? (selected as TextOverlay) : null);
  const selShape = $derived(selected?.type === "shape" ? (selected as ShapeOverlay) : null);
</script>

<PanelSection title="Layers" flush>
  {#if layers.length === 0}
    <p class="text-muted-foreground px-0.5 py-2 text-xs">
      No layers yet. Add text or a shape from the Design tab and it shows up here.
    </p>
  {:else}
    <ul class="flex flex-col gap-1">
      {#each layers as layer, i (layer.id)}
        {@const selected = editor.selectedId === layer.id}
        <li
          class={cn(
            "group/layer flex items-center gap-1.5 rounded-md border px-1.5 py-1 transition-colors",
            selected ? "border-primary bg-primary/5" : "border-border/50 hover:bg-muted/50",
          )}
        >
          <button
            type="button"
            class="flex min-w-0 flex-1 items-center gap-2 text-left outline-none"
            onclick={() => editor.selectOverlay(selected ? null : layer.id)}
            aria-pressed={selected}
          >
            <span class="text-muted-foreground flex size-5 shrink-0 items-center justify-center">
              {#if layer.type === "text"}
                <Type class="size-3.5" />
              {:else if layer.shape === "ellipse"}
                <Circle class="size-3.5" />
              {:else}
                <Square class="size-3.5" />
              {/if}
            </span>
            <span class="text-foreground truncate text-xs">{layerLabel(layer)}</span>
          </button>

          <div class="flex shrink-0 items-center">
            <Button
              variant="ghost"
              size="icon"
              class="size-6"
              aria-label="Move up"
              disabled={i === 0}
              onclick={() => editor.moveOverlay(layer.id, 1)}
            >
              <ArrowUp class="size-3.5" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              class="size-6"
              aria-label="Move down"
              disabled={i === layers.length - 1}
              onclick={() => editor.moveOverlay(layer.id, -1)}
            >
              <ArrowDown class="size-3.5" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              class="text-muted-foreground hover:text-destructive size-6"
              aria-label="Delete layer"
              onclick={() => editor.removeOverlay(layer.id)}
            >
              <Trash2 class="size-3.5" />
            </Button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</PanelSection>

{#if selected}
  {@const sel = selected}
  <PanelSection title="Properties">
    <SliderControl
      label="Rotation"
      value={sel.rotation}
      min={-180}
      max={180}
      step={1}
      unit="°"
      onchange={(v) => editor.updateOverlay(sel.id, { rotation: v })}
    />
    {#if selText}
      {@const t = selText}
      <SliderControl
        label="Font size"
        value={t.fontSize}
        min={8}
        max={120}
        step={1}
        unit="px"
        onchange={(v) => editor.updateOverlay(t.id, { fontSize: v })}
      />
    {:else if selShape}
      {@const s = selShape}
      <SliderControl
        label="Width"
        value={s.w}
        min={5}
        max={100}
        step={1}
        unit="%"
        onchange={(v) => editor.updateOverlay(s.id, { w: v })}
      />
      <SliderControl
        label="Height"
        value={s.h}
        min={5}
        max={100}
        step={1}
        unit="%"
        onchange={(v) => editor.updateOverlay(s.id, { h: v })}
      />
    {/if}
  </PanelSection>
{/if}
