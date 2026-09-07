<script lang="ts">
import { ChevronDown, Layout2, Ratio, X } from "@recast/icons";
import * as DropdownMenu from "@recast/ui/dropdown-menu";
import { onMount } from "svelte";
import type { OutputAspect } from "../../lib/editor/render-state";
import { registerShortcutHandlers } from "../../lib/host-hooks";
import type { EditorStore } from "../../stores/editor-store.svelte";
import PresetPicker from "../PresetPicker.svelte";
import { commitLook, type PresetLook, previewLook } from "../preset-look";
import { type Preset, PRESETS } from "../presets.data";
import { STAGE_PILL } from "./player-bar.styles";

let { store }: { store: EditorStore } = $props();

const OPTIONS: { id: OutputAspect; label: string }[] = [
	{ id: "source", label: "Source" },
	{ id: "16:9", label: "Landscape · 16:9" },
	{ id: "9:16", label: "Portrait · 9:16" },
	{ id: "1:1", label: "Square · 1:1" },
	{ id: "1.91:1", label: "Wide · 1.91:1" },
];

const current = $derived(store.outputAspect);
const activePreset = $derived.by(() => {
	const id = store.lastAppliedPresetId;
	return id ? (PRESETS.find((p) => p.id === id) ?? null) : null;
});

let showPresetsPicker = $state(false);

// Mod+P via the central registry, so no per-component window listener leaks under HMR.
onMount(() =>
	registerShortcutHandlers({
		"editor.presets": () => {
			showPresetsPicker = !showPresetsPicker;
		},
	}),
);

function setAspect(a: OutputAspect) {
	if (a === store.outputAspect) return;
	store.pushUndoState();
	store.outputAspect = a;
}

function readLook(): PresetLook {
	return {
		bg: store.backgroundType,
		value: store.backgroundValue,
		padding: store.padding,
		blur: store.backgroundBlur,
		layout: store.layoutMode,
		aspect: store.outputAspect,
		presetId: store.lastAppliedPresetId,
	};
}
function writeLook(look: PresetLook) {
	store.setBackground({ type: look.bg, value: look.value });
	store.padding = look.padding;
	store.backgroundBlur = look.blur;
	store.layoutMode = look.layout;
	store.outputAspect = look.aspect;
	store.lastAppliedPresetId = look.presetId;
}

// Captured on the first preview so browsing presets leaves undo history clean.
let lookBeforePreview: PresetLook | null = null;
function previewPreset(preset: Preset) {
	lookBeforePreview ??= readLook();
	store.withoutUndo(() => writeLook(previewLook(preset, lookBeforePreview as PresetLook)));
}
function restoreBeforePreview() {
	const before = lookBeforePreview;
	lookBeforePreview = null;
	if (before) store.withoutUndo(() => writeLook(before));
}
function applyPreset(preset: Preset) {
	const before = lookBeforePreview ?? readLook();
	lookBeforePreview = null;
	store.withoutUndo(() => writeLook(before));
	store.pushUndoState();
	store.withoutUndo(() => writeLook(commitLook(preset, before)));
}
function clearPreset() {
	if (store.outputAspect === "source" && store.lastAppliedPresetId === null) return;
	store.pushUndoState();
	store.outputAspect = "source";
	store.lastAppliedPresetId = null;
}
</script>

<DropdownMenu.Root>
  <DropdownMenu.Trigger>
    {#snippet child({ props })}
      <button {...props} type="button" class={STAGE_PILL} aria-label="Composition format">
        <Ratio size={13} class="text-muted-foreground" />
        <span class="font-mono tabular-nums">{current === "source" ? "Source" : current}</span>
        <ChevronDown size={12} class="text-muted-foreground" />
      </button>
    {/snippet}
  </DropdownMenu.Trigger>
  <DropdownMenu.Content size="sm" align="start" class="w-48 text-[11px]">
    <DropdownMenu.Item onSelect={() => (showPresetsPicker = true)}>
      <Layout2 size={13} />
      Presets…
      {#if activePreset}
        <span class="ml-auto truncate text-muted-foreground">{activePreset.label}</span>
      {/if}
    </DropdownMenu.Item>
    {#if activePreset || current !== "source"}
      <DropdownMenu.Item onSelect={clearPreset}>
        <X size={13} />
        Clear
      </DropdownMenu.Item>
    {/if}
    <DropdownMenu.Separator />
    <DropdownMenu.Label>Aspect ratio</DropdownMenu.Label>
    {#each OPTIONS as o (o.id)}
      <DropdownMenu.CheckboxItem checked={current === o.id} onCheckedChange={() => setAspect(o.id)}>
        {o.label}
      </DropdownMenu.CheckboxItem>
    {/each}
  </DropdownMenu.Content>
</DropdownMenu.Root>

<PresetPicker
  open={showPresetsPicker}
  onOpenChange={(v) => (showPresetsPicker = v)}
  onapply={applyPreset}
  onpreview={previewPreset}
  onrestore={restoreBeforePreview}
  currentId={store.lastAppliedPresetId}
/>
