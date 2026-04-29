<script lang="ts">
  import { normaliseBox } from "$lib/annotations/uv";
  import type {
    Annotation,
    EditorStore,
  } from "$lib/stores/editor-store.svelte";
  import {
    AlignCenter as AlignCenterX,
    AlignEndHorizontal,
    AlignEndVertical,
    AlignHorizontalSpaceAround,
    AlignStartHorizontal,
    AlignStartVertical,
    AlignVerticalSpaceAround,
  } from "@lucide/svelte";
  import { cn } from "@recast/ui/utils";

  interface Props {
    store: EditorStore;
    annotation: Annotation;
  }

  let { store, annotation }: Props = $props();

  function fmt(n: number) {
    return (n * 100).toFixed(2);
  }

  function parseAndCommit(value: string, fallback: number): number {
    const parsed = parseFloat(value);
    if (Number.isNaN(parsed)) return fallback;
    return parsed / 100;
  }

  function setBox(updates: Partial<{ x: number; y: number; w: number; h: number }>) {
    if (
      annotation.kind.kind === "rect" ||
      annotation.kind.kind === "ellipse" ||
      annotation.kind.kind === "text" ||
      annotation.kind.kind === "image"
    ) {
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
    if (annotation.kind.kind === "arrow") {
      const k = annotation.kind;
      let dx = 0;
      let dy = 0;
      if (axis === "x") {
        const target = anchor === "start" ? 0 : anchor === "end" ? 1 - box.w : 0.5 - box.w / 2;
        dx = target - box.x;
      } else {
        const target = anchor === "start" ? 0 : anchor === "end" ? 1 - box.h : 0.5 - box.h / 2;
        dy = target - box.y;
      }
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

    if (
      annotation.kind.kind !== "rect" &&
      annotation.kind.kind !== "ellipse" &&
      annotation.kind.kind !== "text" &&
      annotation.kind.kind !== "image"
    ) {
      return;
    }
    const updates: Partial<{ x: number; y: number }> = {};
    if (axis === "x") {
      const target = anchor === "start" ? 0 : anchor === "end" ? 1 - box.w : 0.5 - box.w / 2;
      updates.x = target;
    } else {
      const target = anchor === "start" ? 0 : anchor === "end" ? 1 - box.h : 0.5 - box.h / 2;
      updates.y = target;
    }
    store.updateAnnotation(annotation.id, {
      kind: { ...annotation.kind, ...updates },
    });
  }

  // Render geometry inputs depending on the kind. Arrows use endpoint pairs.
  const isArrow = $derived(annotation.kind.kind === "arrow");

  const ALIGN_BTN =
    "grid size-7 place-items-center rounded border border-border bg-background text-muted-foreground transition-colors hover:bg-muted/50 hover:text-foreground";
</script>

<section class="flex flex-col gap-2 border-t border-border pt-3">
  <h3 class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
    Geometry
  </h3>

  {#if isArrow && annotation.kind.kind === "arrow"}
    {@const k = annotation.kind}
    <div class="grid grid-cols-2 gap-2">
      <label class="flex flex-col gap-0.5 text-[10px] text-muted-foreground">
        <span>x1</span>
        <input
          type="number"
          step="0.5"
          value={fmt(k.x1)}
          onchange={(e) =>
            setArrow({ x1: parseAndCommit((e.currentTarget as HTMLInputElement).value, k.x1) })}
          class="h-7 rounded border border-border bg-background px-2 text-[11px] tabular-nums text-foreground outline-none focus:border-primary/60"
        />
      </label>
      <label class="flex flex-col gap-0.5 text-[10px] text-muted-foreground">
        <span>y1</span>
        <input
          type="number"
          step="0.5"
          value={fmt(k.y1)}
          onchange={(e) =>
            setArrow({ y1: parseAndCommit((e.currentTarget as HTMLInputElement).value, k.y1) })}
          class="h-7 rounded border border-border bg-background px-2 text-[11px] tabular-nums text-foreground outline-none focus:border-primary/60"
        />
      </label>
      <label class="flex flex-col gap-0.5 text-[10px] text-muted-foreground">
        <span>x2</span>
        <input
          type="number"
          step="0.5"
          value={fmt(k.x2)}
          onchange={(e) =>
            setArrow({ x2: parseAndCommit((e.currentTarget as HTMLInputElement).value, k.x2) })}
          class="h-7 rounded border border-border bg-background px-2 text-[11px] tabular-nums text-foreground outline-none focus:border-primary/60"
        />
      </label>
      <label class="flex flex-col gap-0.5 text-[10px] text-muted-foreground">
        <span>y2</span>
        <input
          type="number"
          step="0.5"
          value={fmt(k.y2)}
          onchange={(e) =>
            setArrow({ y2: parseAndCommit((e.currentTarget as HTMLInputElement).value, k.y2) })}
          class="h-7 rounded border border-border bg-background px-2 text-[11px] tabular-nums text-foreground outline-none focus:border-primary/60"
        />
      </label>
    </div>
  {:else if annotation.kind.kind === "rect" || annotation.kind.kind === "ellipse" || annotation.kind.kind === "text" || annotation.kind.kind === "image"}
    {@const k = annotation.kind}
    <div class="grid grid-cols-2 gap-2">
      <label class="flex flex-col gap-0.5 text-[10px] text-muted-foreground">
        <span>X</span>
        <input
          type="number"
          step="0.5"
          value={fmt(k.x)}
          onchange={(e) =>
            setBox({ x: parseAndCommit((e.currentTarget as HTMLInputElement).value, k.x) })}
          class="h-7 rounded border border-border bg-background px-2 text-[11px] tabular-nums text-foreground outline-none focus:border-primary/60"
        />
      </label>
      <label class="flex flex-col gap-0.5 text-[10px] text-muted-foreground">
        <span>Y</span>
        <input
          type="number"
          step="0.5"
          value={fmt(k.y)}
          onchange={(e) =>
            setBox({ y: parseAndCommit((e.currentTarget as HTMLInputElement).value, k.y) })}
          class="h-7 rounded border border-border bg-background px-2 text-[11px] tabular-nums text-foreground outline-none focus:border-primary/60"
        />
      </label>
      <label class="flex flex-col gap-0.5 text-[10px] text-muted-foreground">
        <span>W</span>
        <input
          type="number"
          step="0.5"
          value={fmt(k.w)}
          onchange={(e) =>
            setBox({ w: parseAndCommit((e.currentTarget as HTMLInputElement).value, k.w) })}
          class="h-7 rounded border border-border bg-background px-2 text-[11px] tabular-nums text-foreground outline-none focus:border-primary/60"
        />
      </label>
      <label class="flex flex-col gap-0.5 text-[10px] text-muted-foreground">
        <span>H</span>
        <input
          type="number"
          step="0.5"
          value={fmt(k.h)}
          onchange={(e) =>
            setBox({ h: parseAndCommit((e.currentTarget as HTMLInputElement).value, k.h) })}
          class="h-7 rounded border border-border bg-background px-2 text-[11px] tabular-nums text-foreground outline-none focus:border-primary/60"
        />
      </label>
    </div>
  {/if}

  <!-- Frame-relative alignment -->
  <div class="space-y-1">
    <p class="text-[10px] text-muted-foreground">Align to frame</p>
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
      <span class="mx-1 h-4 w-px bg-border"></span>
      <button type="button" onclick={() => alignFrame("y", "start")} class={cn(ALIGN_BTN)} title="Align top">
        <AlignStartHorizontal size={12} />
      </button>
      <button type="button" onclick={() => alignFrame("y", "center")} class={cn(ALIGN_BTN)} title="Center vertically">
        <AlignVerticalSpaceAround size={12} />
      </button>
      <button type="button" onclick={() => alignFrame("y", "end")} class={cn(ALIGN_BTN)} title="Align bottom">
        <AlignEndHorizontal size={12} />
      </button>
      <!-- Reserve a spot for distribute helpers (multi-select) in v2.1 — keeps
           the UI stable as we expand. -->
      <span class="ml-auto text-[10px] text-muted-foreground/60" title="Distribute coming with multi-select">
        <AlignHorizontalSpaceAround size={12} class="opacity-30" />
      </span>
    </div>
  </div>
</section>
