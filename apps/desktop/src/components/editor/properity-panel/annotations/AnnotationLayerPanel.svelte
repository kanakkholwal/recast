<script lang="ts">
  import { kindIcon, kindLabel } from "$lib/annotations/kind-label";
  import type {
    Annotation,
    EditorStore,
  } from "$lib/stores/editor-store.svelte";
  import {
    Copy,
    Eye,
    EyeOff,
    GripVertical,
    Lock,
    Trash2,
    Unlock,
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { cn } from "@recast/ui/utils";

  interface Props {
    store: EditorStore;
  }

  let { store }: Props = $props();

  // Reverse so the topmost (highest z) renders at the top of the panel —
  // matches the convention in Photoshop / Figma / After Effects.
  const ordered = $derived([...store.annotationsByZ].reverse());

  let renamingId = $state<string | null>(null);
  let dragId = $state<string | null>(null);
  let dragOverId = $state<string | null>(null);

  function fmtTime(sec: number): string {
    const s = Math.max(0, sec);
    const m = Math.floor(s / 60);
    const rem = s - m * 60;
    return `${m}:${rem.toFixed(1).padStart(4, "0")}`;
  }

  function startRename(a: Annotation) {
    if (a.locked) return;
    renamingId = a.id;
  }

  function commitRename(a: Annotation, el: HTMLElement) {
    const next = el.innerText.trim();
    store.renameAnnotation(a.id, next);
    renamingId = null;
  }

  function handleRenameKey(e: KeyboardEvent, a: Annotation) {
    const el = e.currentTarget as HTMLElement;
    if (e.key === "Enter") {
      e.preventDefault();
      el.blur();
    } else if (e.key === "Escape") {
      e.preventDefault();
      el.innerText = kindLabel(a);
      el.blur();
    }
  }

  function handleHover(id: string | null) {
    store.hoveredAnnotationId = id;
  }

  // Pointer-driven drag reorder. We commit by passing the new id list to
  // `setAnnotationZOrder` so the store updates z values in one shot.
  function handleDragStart(e: DragEvent, a: Annotation) {
    if (a.locked) {
      e.preventDefault();
      return;
    }
    dragId = a.id;
    e.dataTransfer?.setData("text/plain", a.id);
    e.dataTransfer!.effectAllowed = "move";
  }

  function handleDragOver(e: DragEvent, target: Annotation) {
    if (!dragId) return;
    e.preventDefault();
    e.dataTransfer!.dropEffect = "move";
    dragOverId = target.id;
  }

  function handleDragLeave(target: Annotation) {
    if (dragOverId === target.id) dragOverId = null;
  }

  function handleDrop(e: DragEvent, target: Annotation) {
    if (!dragId || dragId === target.id) {
      dragId = null;
      dragOverId = null;
      return;
    }
    e.preventDefault();

    // Build the new visual order, then translate it back to the store's
    // bottom-up z order before committing.
    const visual = ordered.map((a) => a.id);
    const fromIdx = visual.indexOf(dragId);
    const toIdx = visual.indexOf(target.id);
    if (fromIdx === -1 || toIdx === -1) return;
    const next = [...visual];
    const [moved] = next.splice(fromIdx, 1);
    next.splice(toIdx, 0, moved);
    // Visual order is top → bottom; store wants bottom → top z order.
    store.setAnnotationZOrder([...next].reverse());
    dragId = null;
    dragOverId = null;
  }

  function handleDragEnd() {
    dragId = null;
    dragOverId = null;
  }
</script>

<section class="flex flex-col gap-1">
  <header class="flex items-center justify-between">
    <h3 class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
      Layers
    </h3>
    <span class="text-[10px] tabular-nums text-muted-foreground/70">
      {ordered.length}
    </span>
  </header>

  {#each ordered as a (a.id)}
    {@const Icon = kindIcon(a)}
    {@const isActive = a.id === store.selectedAnnotationId}
    {@const isRenaming = renamingId === a.id}
    {@const isDragging = dragId === a.id}
    {@const isOverThis = dragOverId === a.id && dragId !== a.id}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      role="button"
      tabindex="0"
      draggable={!isRenaming && !a.locked}
      ondragstart={(e) => handleDragStart(e, a)}
      ondragover={(e) => handleDragOver(e, a)}
      ondragleave={() => handleDragLeave(a)}
      ondrop={(e) => handleDrop(e, a)}
      ondragend={handleDragEnd}
      onclick={() => (store.selectedAnnotationId = a.id)}
      onkeydown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          store.selectedAnnotationId = a.id;
        }
      }}
      onmouseenter={() => handleHover(a.id)}
      onmouseleave={() => handleHover(null)}
      onfocusin={() => handleHover(a.id)}
      onfocusout={() => handleHover(null)}
      class={cn(
        "group relative flex items-center gap-1.5 rounded-md border px-1.5 py-1.5 transition-all",
        "focus:outline-none focus-visible:ring-1 focus-visible:ring-ring",
        isActive
          ? "border-primary/30 bg-primary/8"
          : "border-border/40 bg-card/40 hover:bg-muted/40",
        isDragging && "opacity-40",
        isOverThis && "ring-1 ring-primary/40",
        a.hidden && "opacity-60",
      )}
      data-annotation-row={a.id}
    >
      {#if isActive}
        <span class="absolute inset-y-1 left-0 w-0.5 rounded-full bg-primary" aria-hidden="true"></span>
      {/if}

      <span
        class="flex size-4 shrink-0 cursor-grab items-center justify-center text-muted-foreground/50 transition-colors group-hover:text-muted-foreground active:cursor-grabbing"
        aria-hidden="true"
      >
        <GripVertical size={11} />
      </span>

      <span
        class={cn(
          "grid size-5 shrink-0 place-items-center rounded text-[10px]",
          isActive ? "bg-primary/15 text-primary" : "bg-muted/60 text-muted-foreground",
        )}
      >
        <Icon size={11} />
      </span>

      <div class="flex min-w-0 flex-1 flex-col gap-0">
        {#if isRenaming}
          <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
          <span
            role="textbox"
            tabindex="0"
            contenteditable="true"
            spellcheck="false"
            class="truncate rounded bg-background px-1 text-[11px] font-medium text-foreground outline-1 outline-primary/50"
            onblur={(e) => commitRename(a, e.currentTarget as HTMLElement)}
            onkeydown={(e) => handleRenameKey(e, a)}
          >{kindLabel(a)}</span>
        {:else}
          <button
            type="button"
            class="truncate text-left text-[11px] font-medium text-foreground"
            ondblclick={(e) => {
              e.stopPropagation();
              startRename(a);
            }}
            title="Double-click to rename"
          >{kindLabel(a)}</button>
        {/if}
        <span class="text-[10px] tabular-nums text-muted-foreground">
          {fmtTime(a.start)}–{fmtTime(a.end)}
        </span>
      </div>

      <div
        class="flex shrink-0 items-center gap-0.5 opacity-60 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100"
      >
        <Button
          variant="ghost"
          size="icon-xs"
          onclick={(e) => {
            e.stopPropagation();
            store.toggleAnnotationVisibility(a.id);
          }}
          title={a.hidden ? "Show layer" : "Hide layer"}
          aria-pressed={a.hidden}
        >
          {#if a.hidden}
            <EyeOff size={11} />
          {:else}
            <Eye size={11} />
          {/if}
        </Button>
        <Button
          variant="ghost"
          size="icon-xs"
          onclick={(e) => {
            e.stopPropagation();
            store.toggleAnnotationLock(a.id);
          }}
          title={a.locked ? "Unlock layer" : "Lock layer"}
          aria-pressed={a.locked}
        >
          {#if a.locked}
            <Lock size={11} class="text-warning" />
          {:else}
            <Unlock size={11} />
          {/if}
        </Button>
        <Button
          variant="ghost"
          size="icon-xs"
          onclick={(e) => {
            e.stopPropagation();
            store.duplicateAnnotation(a.id);
          }}
          title="Duplicate (⌘D)"
        >
          <Copy size={11} />
        </Button>
        <Button
          variant="ghost"
          size="icon-xs"
          onclick={(e) => {
            e.stopPropagation();
            store.removeAnnotation(a.id);
          }}
          title="Delete"
          class="hover:text-destructive"
        >
          <Trash2 size={11} />
        </Button>
      </div>
    </div>
  {/each}
</section>
