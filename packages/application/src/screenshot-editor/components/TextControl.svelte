<script lang="ts" module>
  import type { ScreenshotEditorState } from "../editor.svelte";

  export interface TextControlProps {
    editor: ScreenshotEditorState;
  }

  const FONTS = [
    { id: "sans", label: "Sans", css: "Inter, system-ui, sans-serif" },
    { id: "serif", label: "Serif", css: "Georgia, 'Times New Roman', serif" },
    { id: "mono", label: "Mono", css: "'Geist Mono', ui-monospace, monospace" },
  ];
  const WEIGHTS = [
    { label: "Regular", value: 400 },
    { label: "Semibold", value: 600 },
    { label: "Bold", value: 700 },
  ];
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { SliderControl } from "@recast/ui/slider-control";
  import { ColorField } from "@recast/ui/color-field";
  import { Button } from "@recast/ui/button";
  import { AlignCenter, AlignLeft, AlignRight, Plus, Trash2, Type } from "@lucide/svelte";
  import type { TextAlign, TextOverlay } from "../types";

  let { editor }: TextControlProps = $props();

  // Only a selected TEXT overlay shows the property editor.
  const selected = $derived(
    editor.selectedOverlay?.type === "text" ? (editor.selectedOverlay as TextOverlay) : null,
  );

  const ALIGNS: { value: TextAlign; icon: typeof AlignLeft }[] = [
    { value: "left", icon: AlignLeft },
    { value: "center", icon: AlignCenter },
    { value: "right", icon: AlignRight },
  ];

  function update(patch: Partial<TextOverlay>) {
    if (selected) editor.updateOverlay(selected.id, patch);
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
    <div class="grid grid-cols-3 gap-1.5">
      {#each FONTS as font (font.id)}
        <button
          type="button"
          class="rounded-lg border px-2 py-1.5 text-xs font-medium transition"
          class:bg-primary={sel.fontFamily === font.css}
          class:text-primary-foreground={sel.fontFamily === font.css}
          class:border-transparent={sel.fontFamily === font.css}
          class:bg-card={sel.fontFamily !== font.css}
          class:border-border={sel.fontFamily !== font.css}
          class:hover:bg-muted={sel.fontFamily !== font.css}
          style:font-family={font.css}
          onclick={() => update({ fontFamily: font.css })}
        >
          {font.label}
        </button>
      {/each}
    </div>

    <SliderControl
      label="Size"
      value={sel.fontSize}
      min={10}
      max={160}
      step={1}
      unit="px"
      onchange={(v) => update({ fontSize: v })}
    />

    <div class="grid grid-cols-3 gap-1.5">
      {#each WEIGHTS as w (w.value)}
        <button
          type="button"
          class="rounded-lg border px-2 py-1.5 text-xs transition"
          class:bg-primary={sel.fontWeight === w.value}
          class:text-primary-foreground={sel.fontWeight === w.value}
          class:border-transparent={sel.fontWeight === w.value}
          class:bg-card={sel.fontWeight !== w.value}
          class:border-border={sel.fontWeight !== w.value}
          class:hover:bg-muted={sel.fontWeight !== w.value}
          style:font-weight={w.value}
          onclick={() => update({ fontWeight: w.value })}
        >
          {w.label}
        </button>
      {/each}
    </div>

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

    <ColorField label="Color" value={sel.color} oncommit={(c) => update({ color: c })} />

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
