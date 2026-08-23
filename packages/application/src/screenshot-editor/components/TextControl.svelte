<script lang="ts" module>
import type { ScreenshotEditorState } from "../editor.svelte";

export interface TextControlProps {
	editor: ScreenshotEditorState;
}

/** Quick color chips (mirror the reference's 8 swatches). */
const SWATCHES = [
	"#ffffff",
	"#000000",
	"#ef4444",
	"#f97316",
	"#eab308",
	"#22c55e",
	"#3b82f6",
	"#8b5cf6",
];
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { SliderControl } from "@recast/ui/slider-control";
  import { SegmentedToggle } from "@recast/ui/segmented";
  import { ColorField } from "@recast/ui/color-field";
  import { Button } from "@recast/ui/button";
  import { AlignCenter, AlignLeft, AlignRight, Plus, Trash2, Type } from "@recast/icons";
  import type { TextAlign, TextOverlay } from "../types";
  import {
    FONT_CATEGORY_LABELS,
    fontCss,
    fontWeights,
    fontsByCategory,
    type FontCategory,
  } from "../fonts";

  let { editor }: TextControlProps = $props();

  const selected = $derived(
    editor.selectedOverlay?.type === "text" ? (editor.selectedOverlay as TextOverlay) : null,
  );

  const ALIGNS: { value: TextAlign; icon: typeof AlignLeft }[] = [
    { value: "left", icon: AlignLeft },
    { value: "center", icon: AlignCenter },
    { value: "right", icon: AlignRight },
  ];

  const CATEGORY_ORDER: FontCategory[] = [
    "sans-serif",
    "display",
    "serif",
    "handwriting",
    "monospace",
    "system",
  ];

  const weightLabel = (w: string) =>
    w === "normal" ? "Regular" : w === "bold" ? "Bold" : w;

  function update(patch: Partial<TextOverlay>) {
    if (selected) editor.updateOverlay(selected.id, patch);
  }

  // Switching font may invalidate the current weight; snap to the nearest valid.
  function pickFont(id: string) {
    const weights = fontWeights(id);
    const keep = selected && weights.includes(selected.fontWeight);
    update({ fontFamily: id, fontWeight: keep ? selected.fontWeight : "normal" });
  }
</script>

<PanelSection title="Text">
  {#snippet action()}
    <Button variant="ghost" size="xs" onclick={() => editor.addText()}>
      <Plus />
      Add
    </Button>
  {/snippet}

  {#if selected}
    {@const sel = selected}
    <label class="flex flex-col gap-1">
      <span class="text-muted-foreground text-xs font-medium">Font</span>
      <select
        class="border-border bg-card focus-visible:ring-ring h-9 rounded-lg border px-2 text-sm focus-visible:ring-2 focus-visible:outline-none"
        style:font-family={fontCss(sel.fontFamily)}
        value={sel.fontFamily}
        onchange={(e) => pickFont(e.currentTarget.value)}
      >
        {#each CATEGORY_ORDER as cat (cat)}
          <optgroup label={FONT_CATEGORY_LABELS[cat]}>
            {#each fontsByCategory(cat) as f (f.id)}
              <option value={f.id}>{f.name}</option>
            {/each}
          </optgroup>
        {/each}
      </select>
    </label>

    <div class="flex gap-2">
      <label class="flex flex-1 flex-col gap-1">
        <span class="text-muted-foreground text-xs font-medium">Weight</span>
        <select
          class="border-border bg-card focus-visible:ring-ring h-9 rounded-lg border px-2 text-sm focus-visible:ring-2 focus-visible:outline-none"
          value={sel.fontWeight}
          onchange={(e) => update({ fontWeight: e.currentTarget.value })}
        >
          {#each fontWeights(sel.fontFamily) as w (w)}
            <option value={w}>{weightLabel(w)}</option>
          {/each}
        </select>
      </label>
    </div>

    <SliderControl
      label="Size"
      value={sel.fontSize}
      min={8}
      max={150}
      step={1}
      unit="px"
      onchange={(v) => update({ fontSize: v })}
    />

    <div class="flex gap-1.5">
      {#each ALIGNS as a (a.value)}
        {@const Icon = a.icon}
        <button
          type="button"
          class="flex flex-1 items-center justify-center rounded-lg border py-1.5 transition"
          class:bg-primary={sel.align === a.value}
          class:text-primary-foreground={sel.align === a.value}
          class:border-transparent={sel.align === a.value}
          class:bg-card={sel.align !== a.value}
          class:border-border={sel.align !== a.value}
          class:hover:bg-muted={sel.align !== a.value}
          aria-label={`Align ${a.value}`}
          onclick={() => update({ align: a.value })}
        >
          <Icon class="size-4" />
        </button>
      {/each}
    </div>

    <div class="flex items-center justify-between">
      <span class="text-muted-foreground text-xs">Vertical</span>
      <SegmentedToggle
        checked={sel.orientation === "vertical"}
        onCheckedChange={(v) => update({ orientation: v ? "vertical" : "horizontal" })}
        aria-label="Vertical text"
      />
    </div>

    <ColorField label="Color" value={sel.color} oncommit={(c) => update({ color: c })} />
    <div class="flex flex-wrap gap-1.5">
      {#each SWATCHES as c (c)}
        <button
          type="button"
          class="size-5 rounded-full border"
          class:ring-2={sel.color.toLowerCase() === c}
          class:ring-primary={sel.color.toLowerCase() === c}
          style:background-color={c}
          aria-label={`Color ${c}`}
          onclick={() => update({ color: c })}
        ></button>
      {/each}
    </div>

    <SliderControl
      label="Opacity"
      value={Math.round(sel.opacity * 100)}
      min={0}
      max={100}
      step={1}
      unit="%"
      onchange={(v) => update({ opacity: v / 100 })}
    />

    <div class="border-border flex flex-col gap-2 border-t pt-2">
      <div class="flex items-center justify-between">
        <span class="text-muted-foreground text-xs">Text shadow</span>
        <SegmentedToggle
          checked={sel.shadow.enabled}
          onCheckedChange={(v) => update({ shadow: { ...sel.shadow, enabled: v } })}
          aria-label="Text shadow"
        />
      </div>
      {#if sel.shadow.enabled}
        <ColorField
          label="Shadow color"
          value={sel.shadow.color}
          oncommit={(c) => update({ shadow: { ...sel.shadow, color: c } })}
        />
        <SliderControl
          label="Blur"
          value={sel.shadow.blur}
          min={0}
          max={20}
          step={1}
          unit="px"
          onchange={(v) => update({ shadow: { ...sel.shadow, blur: v } })}
        />
        <SliderControl
          label="Offset X"
          value={sel.shadow.offsetX}
          min={-20}
          max={20}
          step={1}
          unit="px"
          onchange={(v) => update({ shadow: { ...sel.shadow, offsetX: v } })}
        />
        <SliderControl
          label="Offset Y"
          value={sel.shadow.offsetY}
          min={-20}
          max={20}
          step={1}
          unit="px"
          onchange={(v) => update({ shadow: { ...sel.shadow, offsetY: v } })}
        />
      {/if}
    </div>

    <Button variant="ghost" size="sm" class="w-full" onclick={() => editor.removeOverlay(sel.id)}>
      <Trash2 />
      Delete text
    </Button>
  {:else}
    <button
      type="button"
      class="border-border text-muted-foreground hover:bg-muted/50 hover:text-foreground flex w-full items-center justify-center gap-2 rounded-lg border border-dashed py-2.5 text-xs font-medium transition"
      onclick={() => editor.addText()}
    >
      <Type class="size-4" />
      Add a text layer
    </button>
  {/if}
</PanelSection>
