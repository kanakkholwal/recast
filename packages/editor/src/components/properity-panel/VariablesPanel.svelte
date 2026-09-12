<script lang="ts">
import { ColorField } from "@recast/ui/color-field";
import { Input } from "@recast/ui/input";
import { Switch } from "@recast/ui/switch";
import type { VarSpec } from "../../lib/editor/render-state";
import type { EditorStore } from "../../stores/editor-store.svelte";
import FontPicker from "./FontPicker.svelte";
import NumberField from "./NumberField.svelte";
import PanelSection from "./PanelSection.svelte";
import PropRow from "./PropRow.svelte";
import PropSelect from "./PropSelect.svelte";
import {
	decimalsFor,
	groupVariables,
	readNumber,
	readVec2,
	selectOptions,
	trimNumber,
	writeVec2,
} from "./variables-panel.logic";

interface Props {
	store: EditorStore;
}

let { store }: Props = $props();

const groups = $derived(groupVariables(store.vars));

function set(spec: VarSpec, value: string) {
	store.setVariable(spec.name, value);
}

/** One entry per gesture: a drag opens it, a typed or clicked value is its own. */
function commit(spec: VarSpec, value: string, viaDrag = false) {
	if (!viaDrag) store.pushUndoState();
	set(spec, value);
}
</script>

{#if store.vars.length === 0}
  <p class="px-1 text-[11px] leading-relaxed text-muted-foreground">
    This project declares no variables. A <code class="text-foreground">&lt;vars&gt;</code> block in
    the project file adds them, and every value that references one lands here.
  </p>
{:else}
  <div class="space-y-4">
    {#each groups as group (group.path)}
      <PanelSection
        title={group.path || "Variables"}
        hint={group.path
          ? undefined
          : "Declared in the project file. Changing one moves every value that references it."}
        collapsible={groups.length > 1}
      >
        {#each group.vars as spec (spec.name)}
          <PropRow label={spec.name} alignTop={spec.type === "vec2"}>
            {#if spec.type === "color"}
              <ColorField
                dense
                hideLabel
                class="flex-1"
                label={spec.name}
                value={spec.value}
                oncommit={(next: string) => commit(spec, next)}
              />
            {:else if spec.type === "bool"}
              <Switch
                checked={spec.value === "true"}
                aria-label={spec.name}
                onCheckedChange={(on: boolean) => commit(spec, on ? "true" : "false")}
              />
            {:else if spec.type === "select"}
              <PropSelect
                class="flex-1"
                label={spec.name}
                value={spec.value}
                options={selectOptions(spec)}
                onChange={(next) => commit(spec, next)}
              />
            {:else if spec.type === "font"}
              <div class="min-w-0 flex-1">
                <FontPicker value={spec.value} onChange={(next) => commit(spec, next)} />
              </div>
            {:else if spec.type === "vec2"}
              {@const pair = readVec2(spec.value)}
              <div class="flex min-w-0 flex-1 gap-1.5">
                <NumberField
                  class="flex-1"
                  label="{spec.name} x"
                  glyph="X"
                  value={pair[0]}
                  step={spec.step ?? 0.01}
                  decimals={decimalsFor(spec)}
                  onDragStart={() => store.pushUndoState()}
                  onInput={(v) => set(spec, writeVec2(v, pair[1]))}
                  onCommit={(v, viaDrag) => commit(spec, writeVec2(v, pair[1]), viaDrag)}
                />
                <NumberField
                  class="flex-1"
                  label="{spec.name} y"
                  glyph="Y"
                  value={pair[1]}
                  step={spec.step ?? 0.01}
                  decimals={decimalsFor(spec)}
                  onDragStart={() => store.pushUndoState()}
                  onInput={(v) => set(spec, writeVec2(pair[0], v))}
                  onCommit={(v, viaDrag) => commit(spec, writeVec2(pair[0], v), viaDrag)}
                />
              </div>
            {:else if spec.type === "number" || spec.type === "int" || spec.type === "angle"}
              <NumberField
                class="flex-1"
                label={spec.name}
                value={readNumber(spec.value)}
                min={spec.min}
                max={spec.max}
                step={spec.step ?? (spec.type === "int" ? 1 : 0.1)}
                decimals={decimalsFor(spec)}
                suffix={spec.type === "angle" ? "°" : undefined}
                onDragStart={() => store.pushUndoState()}
                onInput={(v) => set(spec, trimNumber(v))}
                onCommit={(v, viaDrag) => commit(spec, trimNumber(v), viaDrag)}
              />
            {:else}
              <Input
                class="h-8 min-w-0 flex-1 text-[11px]"
                aria-label={spec.name}
                value={spec.value}
                placeholder={spec.type === "asset" ? "media/…" : ""}
                onchange={(e: Event) => commit(spec, (e.currentTarget as HTMLInputElement).value)}
              />
            {/if}
          </PropRow>
        {/each}
      </PanelSection>
    {/each}
  </div>
{/if}
