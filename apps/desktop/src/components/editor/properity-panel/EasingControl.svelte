<script lang="ts">
import { easingEquals, type Easing } from "$lib/easing/cubic-bezier";
import { registry } from "$lib/registry";
import { Button } from "@recast/ui/button";
import BezierEditor from "../_components/BezierEditor.svelte";
import PanelSection from "./PanelSection.svelte";

interface Props {
	value: Easing;
	/** Discrete pick (preset button). Push undo in the handler. */
	onpick: (next: Easing) => void;
	/**
	 * Continuous edit (dragging a curve handle) — fires per pointermove, so the
	 * handler must coalesce undo rather than pushing per call.
	 */
	ondrag: (next: Easing) => void;
	size?: number;
}

let { value, onpick, ondrag, size = 200 }: Props = $props();

// From the registry, so easing added by an extension pack surfaces here too.
const presets = $derived(
	registry.list("easing").map((e) => ({ id: e.id, label: e.label, value: e.value.value })),
);
</script>

<div class="flex flex-wrap gap-1">
  {#each presets as preset (preset.id)}
    {@const active = easingEquals(value, preset.value)}
    <Button
      type="button"
      size="xs"
      aria-pressed={active}
      variant={active ? "default_soft" : "outline"}
      onclick={() => onpick({ ...preset.value })}
    >
      {preset.label}
    </Button>
  {/each}
</div>

<PanelSection title="Custom curve" flush collapsible defaultOpen={false}>
  <div class="pt-1">
    <BezierEditor value={value} onchange={ondrag} showPresets={false} {size} />
  </div>
</PanelSection>
