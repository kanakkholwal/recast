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
import { cn } from "@recast/ui/utils";
import DraggableValue from "../DraggableValue.svelte";
import PanelSection from "../PanelSection.svelte";
import { alignTarget } from "./annotation-geometry.logic";

interface Props {
	store: EditorStore;
	annotation: Annotation;
}

let { store, annotation }: Props = $props();

function applyBox(updates: Partial<{ x: number; y: number; w: number; h: number }>) {
	if (!isBoxKind(annotation.kind)) return;
	store.updateAnnotation(annotation.id, { kind: { ...annotation.kind, ...updates } });
}

function applyArrow(updates: Partial<{ x1: number; y1: number; x2: number; y2: number }>) {
	if (annotation.kind.kind !== "arrow") return;
	store.updateAnnotation(annotation.id, { kind: { ...annotation.kind, ...updates } });
}

// Field wiring: a drag pushes undo once at its start and previews without undo;
// a typed/keyed edit stays one undo entry per commit, as the old inputs did.
function field(apply: (v: number) => void) {
	return {
		onDragStart: () => store.pushUndoState(),
		onInput: (v: number) => store.withoutUndo(() => apply(v / 100)),
		onCommit: (v: number, viaDrag: boolean) => {
			if (viaDrag) {
				store.withoutUndo(() => apply(v / 100));
			} else {
				store.pushUndoState();
				apply(v / 100);
			}
		},
	};
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

const ALIGN_BTN =
	"grid size-7 place-items-center rounded-md border border-border/60 bg-card/60 text-muted-foreground transition-colors hover:border-border hover:text-foreground focus:outline-none focus:ring-2 focus:ring-ring/40";
</script>

<PanelSection title="Geometry" flush collapsible defaultOpen={false}>
  <div class="flex flex-col gap-2.5">
    {#if isArrow && annotation.kind.kind === "arrow"}
      {@const k = annotation.kind}
      <div class="grid grid-cols-2 gap-1.5">
        <DraggableValue label="X1" value={k.x1 * 100} step={0.5} decimals={1} suffix="%" {...field((v) => applyArrow({ x1: v }))} />
        <DraggableValue label="Y1" value={k.y1 * 100} step={0.5} decimals={1} suffix="%" {...field((v) => applyArrow({ y1: v }))} />
        <DraggableValue label="X2" value={k.x2 * 100} step={0.5} decimals={1} suffix="%" {...field((v) => applyArrow({ x2: v }))} />
        <DraggableValue label="Y2" value={k.y2 * 100} step={0.5} decimals={1} suffix="%" {...field((v) => applyArrow({ y2: v }))} />
      </div>
    {:else if isBoxKind(annotation.kind)}
      {@const k = annotation.kind}
      <div class="grid grid-cols-2 gap-1.5">
        <DraggableValue label="X" value={k.x * 100} step={0.5} decimals={1} suffix="%" {...field((v) => applyBox({ x: v }))} />
        <DraggableValue label="Y" value={k.y * 100} step={0.5} decimals={1} suffix="%" {...field((v) => applyBox({ y: v }))} />
        <DraggableValue label="W" value={k.w * 100} step={0.5} decimals={1} suffix="%" {...field((v) => applyBox({ w: v }))} />
        <DraggableValue label="H" value={k.h * 100} step={0.5} decimals={1} suffix="%" {...field((v) => applyBox({ h: v }))} />
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
