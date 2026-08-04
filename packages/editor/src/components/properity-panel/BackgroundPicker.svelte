<script lang="ts">
import LazyExternalImage from "../common/LazyExternalImage.svelte";
import { getRecentColors, pushRecentColor } from "../../lib/annotations/recent-colors";
import { registry } from "../../lib/registry";
import {
	COLOR_PRESETS,
	GRADIENT_PRESETS,
	MAX_FRAME_PADDING_PERCENT,
	WALLPAPERS,
	wallpaperBackgroundValue,
	type BackgroundType,
	type EditorStore,
} from "../../stores/editor-store.svelte";
import type { IconComponent } from "@recast/icons";
import {
	Blend,
	Check,
	FolderOpen,
	ImageIcon,
	LayoutTemplate,
	Move,
	Palette,
	SquareRoundCorner,
	Wallpaper,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import { ColorField } from "@recast/ui/color-field";
import { SegmentedToggle } from "@recast/ui/segmented";
import { SliderControl } from "@recast/ui/slider-control";
import * as Tabs from "@recast/ui/tabs";
import { cn } from "@recast/ui/utils";
import { getEditorServices } from "../../lib/editor/services";
import { Image } from "@unpic/svelte";
import {
	imagePreviewSrc,
	isValidImageValue,
	selectionValueForType,
} from "./background-picker.logic";
import GradientBuilder from "./GradientBuilder.svelte";
import PanelSection from "./PanelSection.svelte";

interface Props {
	store: EditorStore;
}

type BackgroundMode = {
	type: BackgroundType;
	label: string;
	icon: IconComponent;
};

const backgroundModes: BackgroundMode[] = [
	{ type: "wallpaper", label: "Wallpaper", icon: Wallpaper },
	{ type: "color", label: "Color", icon: Palette },
	{ type: "gradient", label: "Gradient", icon: Blend },
	{ type: "image", label: "Image", icon: ImageIcon },
];

const DEFAULT_BACKGROUND_VALUES: Record<BackgroundType, string> = {
	wallpaper: WALLPAPERS[0] ? wallpaperBackgroundValue(WALLPAPERS[0].id) : "",
	color: COLOR_PRESETS[0]?.value ?? "#000000",
	gradient: GRADIENT_PRESETS[0]?.value ?? "linear-gradient(135deg, #111827 0%, #1f2937 100%)",
	image: "",
};

let { store }: Props = $props();

const services = getEditorServices();

let recents = $state<string[]>(getRecentColors());
function rememberColor(color: string) {
	recents = pushRecentColor(color);
}

// Mode tabs only choose which preset list is shown; they don't mutate the
// background (only an explicit preset pick does), so browsing other modes
// keeps the applied background intact.
// Overridable $derived: re-syncs when the store's type changes, and the tab's
// onValueChange writes over it for browsing. No $effect needed to mirror it.
let displayedMode = $derived<BackgroundType>(store.backgroundType);

let blurValue = $state(0);
let paddingValue = $state(0);
let borderRadiusValue = $state(0);

const isRegisteredBackground = (id: string) => registry.get("background", id) !== undefined;

function getSelectionValue(type: BackgroundType) {
	return selectionValueForType(
		type,
		store.backgroundValue,
		DEFAULT_BACKGROUND_VALUES,
		isRegisteredBackground,
	);
}

function applyBackground(type: BackgroundType, value = getSelectionValue(type)) {
	store.setBackground({ type, value });
}

async function pickBackgroundImage() {
	const selected = await services.pickFile?.({
		accept: ["png", "jpg", "jpeg", "webp"],
		title: "Choose Background Image",
	});
	if (!selected) return;
	store.setBackground({ type: "image", value: selected });
}

// Wrapper: injects the host's resolver into the shared preview helper.
const getImagePreviewSrc = (value: string): string =>
	imagePreviewSrc(value, services.resolveAssetUrl);

$effect(() => {
	blurValue = store.backgroundBlur;
	paddingValue = store.padding;
	borderRadiusValue = store.borderRadius;
});
</script>

<div class="flex flex-col gap-4 animate-in fade-in duration-200">
  <!-- Carries its own contrast so selection reads on any swatch — white, black
       or a gradient — and isn't signalled by colour alone. -->
  {#snippet selectedMark()}
    <span
      class="absolute right-1 top-1 grid size-3.5 place-items-center rounded-full bg-primary text-primary-foreground shadow-sm"
    >
      <Check class="size-2.5" />
    </span>
  {/snippet}

  <!-- Blur only affects texture backgrounds (image/wallpaper), so it lives
       inside those modes rather than as a global knob. -->
  {#snippet blurControl()}
    <SliderControl
      label="Background blur"
      bind:value={blurValue}
      min={0}
      max={100}
      step={1}
      unit="%"
      onstart={() => store.pushUndoState()}
      onchange={(next) => {
        store.backgroundBlur = next;
      }}
    >
      {#snippet icon()}
        <Blend size={11} />
      {/snippet}
    </SliderControl>
  {/snippet}

  <!-- Order follows the dependency chain: choose a background first, then frame
       the recording on it (padding + radius), then the shadow that depth needs
       both of those to read. Background sits first even though it is the taller,
       variable-height section. -->
  <!-- Background mode switcher + per-mode preset lists. The active tab is
       decoupled from the store; it only picks the preset list shown (see
       `displayedMode`). -->
  <Tabs.Root
    value={displayedMode}
    onValueChange={(v: string) => (displayedMode = v as BackgroundType)}
    class="flex flex-col gap-4"
  >
    <PanelSection
      title="Canvas"
      hint="What fills the canvas behind your recording."
      flush
    >
      <Tabs.List
        variant="soft"
        class="flex flex-row h-auto items-center gap-0.5 rounded-lg bg-muted/60 p-0.5 ring-1 ring-inset ring-border/40"
      >
        {#each backgroundModes as mode}
          {@const Icon = mode.icon}
          <Tabs.Trigger
            value={mode.type}
            title={mode.label}
            class="h-6 flex-1 gap-1 px-2 text-[11px] font-medium"
          >
            <!-- `size-` class, not the `size` prop: Tabs.Trigger forces unsized
                 SVGs to size-4, so the class both sets 11px and opts out. -->
            <Icon class="size-2.75" />
            <span class="hidden @[260px]/panel:inline">{mode.label}</span>
          </Tabs.Trigger>
        {/each}
      </Tabs.List>
    </PanelSection>

  <Tabs.Content value="wallpaper">
    <PanelSection title="Wallpapers" flush>
      {#snippet action()}
        <span class="font-mono text-[10px] tabular-nums text-muted-foreground">
          {registry.list("background").length}
        </span>
      {/snippet}
      <div class="grid grid-cols-3 gap-1.5">
        {#each registry.list("background") as entry (entry.id)}
          {@const isSelected = store.backgroundValue === entry.id}
          <Button
            variant="raw"
            size="raw"
            onclick={() => applyBackground("wallpaper", entry.id)}
            class={cn(
              "group relative aspect-video overflow-hidden rounded-md border transition-all",
              isSelected
                ? "border-primary ring-2 ring-primary/30"
                : "border-border hover:border-foreground/30",
            )}
            title={entry.label}
            aria-label="Use {entry.label} background"
            aria-pressed={isSelected}
          >
            {#if entry.thumbUrl}
              <!-- Extension wallpaper: thumbnail already resolved to a WebView URL. -->
              <img
                src={entry.thumbUrl}
                alt={entry.label}
                class="size-full object-cover transition-transform duration-200 group-hover:scale-[1.03]"
              />
            {:else if entry.thumbAssetId}
              <LazyExternalImage
                assetId={entry.thumbAssetId}
                alt={entry.label}
                tier="thumb"
                class="size-full object-cover transition-transform duration-200 group-hover:scale-[1.03]"
              />
            {/if}
            {#if isSelected}
              {@render selectedMark()}
            {/if}
          </Button>
        {/each}
      </div>

      <div class="mt-2.5">
        {@render blurControl()}
      </div>
    </PanelSection>
  </Tabs.Content>

  <Tabs.Content value="color">
    <PanelSection
      title="Color"
      hint="Solid backgrounds keep attention on the recording itself."
      flush
    >
      <div class="grid grid-cols-6 gap-1.5">
        {#each registry.list("color") as entry (entry.id)}
          {@const color = entry.value.value}
          {@const isSelected = store.backgroundValue === color}
          <Button
            variant="raw"
            size="raw"
            onclick={() => applyBackground("color", color)}
            title={entry.label}
            aria-label="Use {entry.label} background"
            aria-pressed={isSelected}
            class={cn(
              "group relative aspect-square overflow-hidden rounded-md border transition-all",
              isSelected
                ? "border-primary ring-2 ring-primary/30"
                : "border-border hover:border-foreground/30",
            )}
            style="background-color: {color}"
          >
            {#if isSelected}
              {@render selectedMark()}
            {/if}
          </Button>
        {/each}
      </div>

      <div class="mt-2">
        <ColorField
          label="Custom"
          value={store.backgroundValue.startsWith("#")
            ? store.backgroundValue
            : DEFAULT_BACKGROUND_VALUES.color}
          {recents}
          oncommit={(c: string) => {
            store.pushUndoState();
            applyBackground("color", c);
            rememberColor(c);
          }}
        />
      </div>
    </PanelSection>
  </Tabs.Content>

  <Tabs.Content value="gradient" class="space-y-4">
    <PanelSection
      title="Gradients"
      hint="Rich preset backdrops, rendered live in the preview and the export."
      flush
    >
      {#snippet action()}
        <span class="font-mono text-[10px] tabular-nums text-muted-foreground">
          {registry.list("gradient").length}
        </span>
      {/snippet}
      <div class="grid grid-cols-3 gap-1.5">
        {#each registry.list("gradient") as entry (entry.id)}
          {@const value = entry.value.value}
          {@const isSelected = store.backgroundValue === value}
          <Button
            variant="raw"
            size="raw"
            onclick={() => applyBackground("gradient", value)}
            class={cn(
              "group relative h-14 overflow-hidden rounded-md border p-1.5 text-left transition-all",
              isSelected
                ? "border-primary ring-2 ring-primary/30"
                : "border-border hover:border-foreground/30",
            )}
            style="background: {value}"
            aria-label="Use {entry.label} gradient"
            aria-pressed={isSelected}
          >
            <div class="flex h-full items-end">
              <!-- Opaque scrim, not black/40: at 40% the label failed 4.5:1 over
                   the lighter gradients (Blush, Sage, Fog). -->
              <span
                class="rounded bg-black/70 px-1.5 py-0.5 text-[10px] font-medium text-white"
              >
                {entry.label}
              </span>
            </div>
            {#if isSelected}
              {@render selectedMark()}
            {/if}
          </Button>
        {/each}
      </div>
    </PanelSection>

    <GradientBuilder {store} {recents} onRememberColor={rememberColor} />
  </Tabs.Content>

  <Tabs.Content value="image">
    <PanelSection
      title="Image"
      hint="Imported images fit to cover the full canvas."
      flush
    >
      {#snippet action()}
        <Button
          variant="outline"
          size="xs"
          class="gap-1.5"
          onclick={pickBackgroundImage}
        >
          <FolderOpen size={11} />
          {store.backgroundValue ? "Replace" : "Choose"}
        </Button>
      {/snippet}
      {#if store.backgroundValue && isValidImageValue(store.backgroundValue)}
        <div
          class="overflow-hidden rounded-md border border-border bg-background"
        >
          <Image
            src={getImagePreviewSrc(store.backgroundValue)}
            alt="Selected background"
            layout="constrained"
            width={320}
            aspectRatio={16 / 9}
            objectFit="cover"
            loading="lazy"
            decoding="async"
            class="max-h-56 w-full"
          />
        </div>
      {:else}
        <div
          class="flex h-20 items-center justify-center rounded-md border border-dashed border-border bg-muted/20 text-[11px] text-muted-foreground"
        >
          No image selected
        </div>
      {/if}

      {#if store.backgroundValue && isValidImageValue(store.backgroundValue)}
        <div class="mt-2.5">
          {@render blurControl()}
        </div>
      {/if}
    </PanelSection>
  </Tabs.Content>
  </Tabs.Root>

  <PanelSection
    title="Frame"
    hint="Padding adds space around the recording; corner radius rounds its edges. Both apply to every background."
  >
    <SliderControl
      label="Frame padding"
      bind:value={paddingValue}
      min={0}
      max={MAX_FRAME_PADDING_PERCENT}
      step={1}
      unit="%"
      onstart={() => store.pushUndoState()}
      onchange={(next) => {
        store.padding = next;
      }}
    >
      {#snippet icon()}
        <LayoutTemplate size={11} />
      {/snippet}
    </SliderControl>

    <SliderControl
      label="Corner radius"
      bind:value={borderRadiusValue}
      min={0}
      max={25}
      step={0.5}
      unit="%"
      onstart={() => store.pushUndoState()}
      onchange={(next) => {
        store.borderRadius = next;
      }}
    >
      {#snippet icon()}
        <SquareRoundCorner size={11} />
      {/snippet}
    </SliderControl>
  </PanelSection>

  <PanelSection
    title="Drop shadow"
    hint="Adds depth by casting a soft shadow under the recording onto the canvas background."
    flush
    collapsible
    defaultOpen={store.shadow.enabled}
  >
    {#snippet action()}
      <SegmentedToggle
        checked={store.shadow.enabled}
        size="xs"
        aria-label="Drop shadow"
        onCheckedChange={(next) => {
          store.pushUndoState();
          store.updateShadow({ enabled: next });
        }}
      />
    {/snippet}

    {#if store.shadow.enabled}
      <div class="space-y-2.5">
        <SliderControl
          label="Blur"
          value={store.shadow.blur}
          min={0}
          max={100}
          step={1}
          unit="px"
          onstart={() => store.pushUndoState()}
          onchange={(v) => store.updateShadow({ blur: v })}
        >
          {#snippet icon()}
            <Blend size={11} />
          {/snippet}
        </SliderControl>

        <SliderControl
          label="Spread"
          value={store.shadow.spread}
          min={0}
          max={50}
          step={1}
          unit="px"
          onstart={() => store.pushUndoState()}
          onchange={(v) => store.updateShadow({ spread: v })}
        >
          {#snippet icon()}
            <SquareRoundCorner size={11} />
          {/snippet}
        </SliderControl>

        <SliderControl
          label="Offset Y"
          value={store.shadow.offsetY}
          min={-40}
          max={40}
          step={1}
          unit="px"
          onstart={() => store.pushUndoState()}
          onchange={(v) => store.updateShadow({ offsetY: v })}
        >
          {#snippet icon()}
            <Move size={11} />
          {/snippet}
        </SliderControl>

        <SliderControl
          label="Opacity"
          value={store.shadow.opacity}
          min={0}
          max={100}
          step={1}
          unit="%"
          onstart={() => store.pushUndoState()}
          onchange={(v) => store.updateShadow({ opacity: v })}
        />

        <ColorField
          label="Shadow color"
          value={store.shadow.color || "#000000"}
          {recents}
          oncommit={(c: string) => {
            store.pushUndoState();
            store.updateShadow({ color: c });
            rememberColor(c);
          }}
        />
      </div>
      {:else}
        <div class="text-[11px] text-muted-foreground">
          Drop shadow is disabled. Enable it to adjust its settings.
        </div>
    {/if}
  </PanelSection>
</div>
