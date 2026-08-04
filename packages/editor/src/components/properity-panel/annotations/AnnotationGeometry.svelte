<script lang="ts">
import { isBoxKind } from "../../../lib/annotations/kind-groups";
import { normaliseBox } from "../../../lib/annotations/uv";
import type { Annotation, EditorStore } from "../../../stores/editor-store.svelte";
import {
	AlignCenter as AlignCenterX,
	AlignEndHorizontal,
	AlignEndVertical,
	AlignStartHorizontal,
	AlignStartVertical,
	AlignVerticalSpaceAround,
} from "@recast/icons";
import { Input } from "@recast/ui/input";
import { cn } from "@recast/ui/utils";
import PanelSection from "../PanelSection.svelte";
import { alignTarget, fmt, parseAndCommit } from "./annotation-geometry.logic";

interface Props {
	store: EditorStore;
	annotation: Annotation;
}

let { store, annotation }: Props = $props();

function setBox(updates: Partial<{ x: number; y: number; w: number; h: number }>) {
	if (isBoxKind(annotation.kind)) {
		store.pushUndoState();
		store.updateAnnotation(annotation.id, {
			kind: { ...annotation.kind, ...updates },
		});
	}
}

function setArrow(updates: Partial<{ x1: number; y1: number; x2: number; y2: number }>) {
	if (annotation.kind.kind !== "arrow") return;
	store.pushUndoState();
	store.updateAnnotation(annotation.id, {
		kind: { ...annotation.kind, ...updates },
	});
}

// Frame-relative alignment. For boxes we move the whole rect; for arrows we
// shift both endpoints by the same delta so direction is preserved.
function alignFrame(axis: "x" | "y", anchor: "start" | "center" | "end") {
	store.pushUndoState();
	const box = normaliseBox(annotation.kind);
	const target = alignTarget(box, axis, anchor);
	if (annotation.kind.kind === "arrow") {
		const k = annotation.kind;
		const dx = axis === "x" ? target - box.x : 0;
		const dy = axis === "y" ? target - box.y : 0;
		store.updateAnnotation(annotation.id, {
			kind: {
				...k,
				x1: k.x1 + dx,
				y1: k.y1 + dy,
				x2: k.x2 + dx,
				y2: k.y2 + dy,
			},
		});
		return;
	}

	if (!isBoxKind(annotation.kind)) return;
	const updates: Partial<{ x: number; y: number }> = axis === "x" ? { x: target } : { y: target };
	store.updateAnnotation(annotation.id, {
		kind: { ...annotation.kind, ...updates },
	});
}

const isArrow = $derived(annotation.kind.kind === "arrow");

const INPUT_CLASS = "h-7 px-2 text-[11px] tabular-nums";
const ALIGN_BTN =
	"grid size-7 place-items-center rounded-md border border-border/60 bg-card/60 text-muted-foreground transition-colors hover:border-border hover:text-foreground focus:outline-none focus:ring-2 focus:ring-ring/40";

const FIELD_LABEL = "flex flex-col gap-0.5 text-[10px] text-muted-foreground";
</script>

<PanelSection title="Geometry" flush collapsible defaultOpen={false}>
  <div class="flex flex-col gap-2.5">
    {#if isArrow && annotation.kind.kind === "arrow"}
      {@const k = annotation.kind}
      <div class="grid grid-cols-2 gap-2">
        <label class={FIELD_LABEL}>
          <span>x1</span>
          <Input
            type="number"
            step="0.5"
            value={fmt(k.x1)}
            onchange={(e) =>
              setArrow({ x1: parseAndCommit((e.currentTarget as HTMLInputElement).value, k.x1) })}
            class={INPUT_CLASS}
          />
        </label>
        <label class={FIELD_LABEL}>
          <span>y1</span>
          <Input
            type="number"
            step="0.5"
            value={fmt(k.y1)}
            onchange={(e) =>
              setArrow({ y1: parseAndCommit((e.currentTarget as HTMLInputElement).value, k.y1) })}
            class={INPUT_CLASS}
          />
        </label>
        <label class={FIELD_LABEL}>
          <span>x2</span>
          <Input
            type="number"
            step="0.5"
            value={fmt(k.x2)}
            onchange={(e) =>
              setArrow({ x2: parseAndCommit((e.currentTarget as HTMLInputElement).value, k.x2) })}
            class={INPUT_CLASS}
          />
        </label>
        <label class={FIELD_LABEL}>
          <span>y2</span>
          <Input
            type="number"
            step="0.5"
            value={fmt(k.y2)}
            onchange={(e) =>
              setArrow({ y2: parseAndCommit((e.currentTarget as HTMLInputElement).value, k.y2) })}
            class={INPUT_CLASS}
          />
        </label>
      </div>
    {:else if isBoxKind(annotation.kind)}
      {@const k = annotation.kind}
      <div class="grid grid-cols-2 gap-2">
        <label class={FIELD_LABEL}>
          <span>X</span>
          <Input
            type="number"
            step="0.5"
            value={fmt(k.x)}
            onchange={(e) =>
              setBox({ x: parseAndCommit((e.currentTarget as HTMLInputElement).value, k.x) })}
            class={INPUT_CLASS}
          />
        </label>
        <label class={FIELD_LABEL}>
          <span>Y</span>
          <Input
            type="number"
            step="0.5"
            value={fmt(k.y)}
            onchange={(e) =>
              setBox({ y: parseAndCommit((e.currentTarget as HTMLInputElement).value, k.y) })}
            class={INPUT_CLASS}
          />
        </label>
        <label class={FIELD_LABEL}>
          <span>W</span>
          <Input
            type="number"
            step="0.5"
            value={fmt(k.w)}
            onchange={(e) =>
              setBox({ w: parseAndCommit((e.currentTarget as HTMLInputElement).value, k.w) })}
            class={INPUT_CLASS}
          />
        </label>
        <label class={FIELD_LABEL}>
          <span>H</span>
          <Input
            type="number"
            step="0.5"
            value={fmt(k.h)}
            onchange={(e) =>
              setBox({ h: parseAndCommit((e.currentTarget as HTMLInputElement).value, k.h) })}
            class={INPUT_CLASS}
          />
        </label>
      </div>
    {/if}

    <div class="flex flex-col gap-1">
      <span class="text-[10px] text-muted-foreground">Align to frame</span>
      <div class="flex items-center gap-1">
        <button type="button" onclick={() => alignFrame("x", "start")} class={cn(ALIGN_BTN)} title="Align left">
          <AlignStartVertical size={12} />
        </button>
        <button type="button" onclick={() => alignFrame("x", "center")} class={cn(ALIGN_BTN)} title="Center horizontally">
          <AlignCenterX size={12} />
        </button>
        <button type="button" onclick={() => alignFrame("x", "end")} class={cn(ALIGN_BTN)} title="Align right">
          <AlignEndVertical size={12} />
        </button>
        <span class="mx-1 h-4 w-px bg-border/60"></span>
        <button type="button" onclick={() => alignFrame("y", "start")} class={cn(ALIGN_BTN)} title="Align top">
          <AlignStartHorizontal size={12} />
        </button>
        <button type="button" onclick={() => alignFrame("y", "center")} class={cn(ALIGN_BTN)} title="Center vertically">
          <AlignVerticalSpaceAround size={12} />
        </button>
        <button type="button" onclick={() => alignFrame("y", "end")} class={cn(ALIGN_BTN)} title="Align bottom">
          <AlignEndHorizontal size={12} />
        </button>
      </div>
    </div>
  </div>
</PanelSection>
