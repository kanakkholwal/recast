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
  import { PropRow } from "@recast/ui/prop-row";
  import { PropSelect } from "@recast/ui/prop-select";
  import { SliderRow } from "@recast/ui/slider-row";
  import * as Select from "@recast/ui/select";
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

  const selectedFontName = $derived.by(() => {
    if (!selected) return "";
    for (const cat of CATEGORY_ORDER) {
      const f = fontsByCategory(cat).find((x) => x.id === selected.fontFamily);
      if (f) return f.name;
    }
    return selected.fontFamily;
  });
  const weightOptions = $derived(
    selected
      ? fontWeights(selected.fontFamily).map((w) => ({ value: w, label: weightLabel(w) }))
      : [],
  );
</script>


<PanelSection variant="panel" title="Text" collapsible defaultOpen>
  {#snippet action()}
    <Button variant="ghost" size="xs" onclick={() => editor.addText()}>
      <Plus />
      Add
    </Button>
  {/snippet}

  {#if selected}
    {@const sel = selected}
    <PropRow label="Font">
      <Select.Root type="single" value={sel.fontFamily} onValueChange={(v) => v && pickFont(v)}>
        <Select.Trigger
          aria-label="Font"
          class="bg-muted/60 ring-border/40 text-foreground hover:bg-muted focus-visible:ring-ring/60 h-8 min-h-0 w-full flex-1 border-transparent py-0 pr-2 pl-2.5 text-xs leading-none font-medium ring-1 ring-inset transition-colors"
        >
          <span class="truncate" style:font-family={fontCss(sel.fontFamily)}>{selectedFontName}</span>
        </Select.Trigger>
        <Select.Content>
          {#each CATEGORY_ORDER as cat (cat)}
            <Select.Group>
              <Select.GroupHeading>{FONT_CATEGORY_LABELS[cat]}</Select.GroupHeading>
              {#each fontsByCategory(cat) as f (f.id)}
                <Select.Item value={f.id} label={f.name}>
                  <span style:font-family={fontCss(f.id)}>{f.name}</span>
                </Select.Item>
              {/each}
            </Select.Group>
          {/each}
        </Select.Content>
      </Select.Root>
    </PropRow>

    <PropRow label="Weight">
      <PropSelect
        class="flex-1"
        label="Font weight"
        value={sel.fontWeight}
        options={weightOptions}
        onChange={(w) => update({ fontWeight: w })}
      />
    </PropRow>

    <SliderRow label="Size" value={sel.fontSize} min={8} max={150} step={1} unit="px" onchange={(v) => update({ fontSize: v })} />

    <div class="flex gap-1.5">
      {#each ALIGNS as a (a.value)}
        {@const Icon = a.icon}
        <button
          type="button"
          class="flex flex-1 items-center justify-center rounded-lg border py-1.5 transition"
          class:bg-foreground={sel.align === a.value}
          class:text-background={sel.align === a.value}
          class:border-transparent={sel.align === a.value}
          class:bg-card={sel.align !== a.value}
          class:border-border={sel.align !== a.value}
          class:hover:bg-accent={sel.align !== a.value}
          aria-label={`Align ${a.value}`}
          onclick={() => update({ align: a.value })}
        >
          <Icon class="size-4" />
        </button>
      {/each}
    </div>

    <PropRow label="Vertical">
      <SegmentedToggle
        checked={sel.orientation === "vertical"}
        onCheckedChange={(v) => update({ orientation: v ? "vertical" : "horizontal" })}
        aria-label="Vertical text"
      />
    </PropRow>

    <PropRow label="Color">
      <ColorField dense hideLabel class="flex-1" label="Text color" value={sel.color} oncommit={(c) => update({ color: c })} />
    </PropRow>
    <div class="flex flex-wrap gap-1.5">
      {#each SWATCHES as c (c)}
        <button
          type="button"
          class="size-6 rounded-full border"
          class:ring-2={sel.color.toLowerCase() === c}
          class:ring-foreground={sel.color.toLowerCase() === c}
          class:ring-offset-1={sel.color.toLowerCase() === c}
          style:background-color={c}
          aria-label={`Color ${c}`}
          onclick={() => update({ color: c })}
        ></button>
      {/each}
    </div>

    <SliderRow label="Opacity" value={Math.round(sel.opacity * 100)} min={0} max={100} step={1} unit="%" onchange={(v) => update({ opacity: v / 100 })} />

    <div class="border-border flex flex-col gap-2 border-t pt-2">
      <PropRow label="Text shadow">
        <SegmentedToggle
          checked={sel.shadow.enabled}
          onCheckedChange={(v) => update({ shadow: { ...sel.shadow, enabled: v } })}
          aria-label="Text shadow"
        />
      </PropRow>
      {#if sel.shadow.enabled}
        <PropRow label="Shadow color">
          <ColorField
            dense
            hideLabel
            class="flex-1"
            label="Shadow color"
            value={sel.shadow.color}
            oncommit={(c) => update({ shadow: { ...sel.shadow, color: c } })}
          />
        </PropRow>
        <SliderRow label="Blur" value={sel.shadow.blur} min={0} max={20} step={1} unit="px" onchange={(v) => update({ shadow: { ...sel.shadow, blur: v } })} />
        <SliderRow label="Offset X" value={sel.shadow.offsetX} min={-20} max={20} step={1} unit="px" onchange={(v) => update({ shadow: { ...sel.shadow, offsetX: v } })} />
        <SliderRow label="Offset Y" value={sel.shadow.offsetY} min={-20} max={20} step={1} unit="px" onchange={(v) => update({ shadow: { ...sel.shadow, offsetY: v } })} />
      {/if}
    </div>

    <Button variant="ghost" size="sm" class="w-full" onclick={() => editor.removeOverlay(sel.id)}>
      <Trash2 />
      Delete text
    </Button>
  {:else}
    <button
      type="button"
      class="border-border text-muted-foreground hover:bg-accent hover:text-foreground flex w-full items-center justify-center gap-2 rounded-lg border border-dashed py-2.5 text-xs font-medium transition-colors"
      onclick={() => editor.addText()}
    >
      <Type class="size-4" />
      Add a text layer
    </button>
  {/if}
</PanelSection>
