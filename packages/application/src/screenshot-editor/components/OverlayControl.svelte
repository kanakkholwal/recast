<script lang="ts" module>
import type { ScreenshotEditorState } from "../editor.svelte";

export interface OverlayControlProps {
	editor: ScreenshotEditorState;
}
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { SliderControl } from "@recast/ui/slider-control";
  import { SegmentedToggle } from "@recast/ui/segmented";
  import { Button } from "@recast/ui/button";
  import { cn } from "@recast/ui/utils";
  import { FlipHorizontal2, ImagePlus, Trash2 } from "@recast/icons";
  import { imageFromFile } from "../image-input";
  import { OVERLAY_SHADOWS } from "../image-backgrounds";
  import type { ImageOverlay } from "../types";

  let { editor }: OverlayControlProps = $props();
  let fileInput = $state<HTMLInputElement | null>(null);

  const selected = $derived(
    editor.selectedOverlay?.type === "image" ? (editor.selectedOverlay as ImageOverlay) : null,
  );

  function update(patch: Partial<ImageOverlay>) {
    if (selected) editor.updateOverlay(selected.id, patch);
  }

  async function onFile(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = "";
    if (!file) return;
    const img = await imageFromFile(file);
    editor.addImageOverlay(img.src, { isCustom: true });
  }
</script>

<input bind:this={fileInput} type="file" accept="image/*" class="hidden" onchange={onFile} />

<PanelSection title="Images">
  {#snippet action()}
    <Button variant="ghost" size="xs" onclick={() => fileInput?.click()}>
      <ImagePlus />
      Upload
    </Button>
  {/snippet}

  {#if selected}
    {@const sel = selected}
    <SliderControl
      label="Size"
      value={sel.size}
      min={5}
      max={100}
      step={1}
      unit="%"
      onchange={(v) => update({ size: v })}
    />
    <SliderControl
      label="Rotation"
      value={sel.rotation}
      min={-180}
      max={180}
      step={1}
      unit="°"
      onchange={(v) => update({ rotation: v })}
    />
    <SliderControl
      label="Opacity"
      value={Math.round(sel.opacity * 100)}
      min={0}
      max={100}
      step={1}
      unit="%"
      onchange={(v) => update({ opacity: v / 100 })}
    />
    <SliderControl
      label="Blur"
      value={sel.blur}
      min={0}
      max={20}
      step={1}
      unit="px"
      onchange={(v) => update({ blur: v })}
    />
    <div class="flex items-center justify-between">
      <span class="text-muted-foreground flex items-center gap-1.5 text-xs">
        <FlipHorizontal2 class="size-3.5" /> Flip X
      </span>
      <SegmentedToggle checked={sel.flipX} onCheckedChange={(v) => update({ flipX: v })} aria-label="Flip horizontal" />
    </div>
    <div class="flex items-center justify-between">
      <span class="text-muted-foreground flex items-center gap-1.5 text-xs">
        <FlipHorizontal2 class="size-3.5 rotate-90" /> Flip Y
      </span>
      <SegmentedToggle checked={sel.flipY} onCheckedChange={(v) => update({ flipY: v })} aria-label="Flip vertical" />
    </div>
    <Button variant="ghost" size="sm" class="w-full" onclick={() => editor.removeOverlay(sel.id)}>
      <Trash2 />
      Delete image
    </Button>
  {:else}
    <button
      type="button"
      class="border-border text-muted-foreground hover:bg-muted/50 hover:text-foreground flex w-full items-center justify-center gap-2 rounded-lg border border-dashed py-2.5 text-xs font-medium transition"
      onclick={() => fileInput?.click()}
    >
      <ImagePlus class="size-4" />
      Add an image or logo
    </button>
  {/if}
</PanelSection>

<!-- Light & Shadow: a SINGLE soft overlay over the shot. Picking one replaces
     the current (never stacks); "None" clears it. Collapsed so its thumbnails
     only fetch when opened. -->
<PanelSection title="Light & Shadow" collapsible defaultOpen={false}>
  <div class="grid grid-cols-4 gap-2">
    <button
      type="button"
      class={cn(
        "border-border text-muted-foreground flex aspect-square items-center justify-center rounded-lg border text-xs font-medium transition-colors",
        editor.shadowOverlay ? "hover:bg-muted" : "border-primary bg-primary/10 text-foreground",
      )}
      aria-pressed={!editor.shadowOverlay}
      onclick={() => editor.setShadowOverlay(null)}
    >
      None
    </button>
    {#each OVERLAY_SHADOWS as shadow (shadow.id)}
      {@const active = editor.shadowOverlay?.src === shadow.url}
      <button
        type="button"
        class={cn(
          "border-border bg-muted/40 aspect-square overflow-hidden rounded-lg border transition-transform hover:scale-105",
          active && "ring-primary ring-2 ring-offset-1",
        )}
        aria-label={shadow.id}
        aria-pressed={active}
        onclick={() => editor.setShadowOverlay(shadow.url)}
      >
        <img src={shadow.url} alt="" loading="lazy" decoding="async" class="size-full object-cover" />
      </button>
    {/each}
  </div>

  {#if editor.shadowOverlay}
    {@const s = editor.shadowOverlay}
    <SliderControl
      label="Shadow opacity"
      value={Math.round(s.opacity * 100)}
      min={0}
      max={100}
      step={1}
      unit="%"
      onchange={(v) => editor.updateOverlay(s.id, { opacity: v / 100 })}
    />
  {/if}
</PanelSection>
