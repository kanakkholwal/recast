<script lang="ts">
import type { IconComponent } from "@recast/icons";
import {
	Briefcase,
	Camera,
	Check,
	Clapperboard,
	CornerDownLeft,
	MessageCircle,
	MonitorPlay,
	Music2,
	Search,
	Sparkles,
	Star,
	X,
} from "@recast/icons";
import { Kbd, KbdGroup } from "@recast/ui/kbd";
import { cn } from "@recast/ui/utils";
import { tick, untrack } from "svelte";
import { cubicOut } from "svelte/easing";
import { prefersReducedMotion } from "svelte/motion";
import { fade, scale } from "svelte/transition";
import LazyExternalImage from "./common/LazyExternalImage.svelte";
import {
	aspectClass,
	bgPreviewStyle,
	buildModel,
	clampIndex,
	filterPresets,
	frameInsetPct,
	groupPresets,
	optionId,
	resolveEscape,
	rowMoveIndex,
	wallpaperId,
} from "./preset-picker.logic";
import { PRESETS, type Preset } from "./presets.data";

interface Props {
	open: boolean;
	onOpenChange: (v: boolean) => void;
	onapply: (preset: Preset) => void;
	/**
	 * Fired as the cursor moves so the editor previews the look. `onapply`
	 * commits; `onrestore` puts the project back when the picker is cancelled.
	 */
	onpreview?: (preset: Preset) => void;
	onrestore?: () => void;
	/** Id of the preset currently applied to the project, if any. */
	currentId?: string | null;
}

let { open, onOpenChange, onapply, onpreview, onrestore, currentId = null }: Props = $props();

let query = $state("");
let selectedIndex = $state(0);
let inputRef = $state<HTMLInputElement | null>(null);
let listRef = $state<HTMLDivElement | null>(null);
let dialogRef = $state<HTMLElement | null>(null);
// The control that opened us, so focus goes home on close.
let returnFocusTo: HTMLElement | null = null;
let previewArmed = $state(false);

const baseId = $props.id();
const COLS = 2;
const GRID_CLASS = "grid grid-cols-2 gap-1";

const currentPreset = $derived(
	currentId ? (PRESETS.find((p) => p.id === currentId) ?? null) : null,
);

const filtered = $derived(filterPresets(PRESETS, query));
const grouped = $derived(groupPresets(filtered, query, currentPreset));
const model = $derived(buildModel(grouped, COLS));
const selectedPreset = $derived(model.flat[selectedIndex] ?? null);

const resultSummary = $derived(
	model.flat.length === 0
		? "No presets match"
		: `${model.flat.length} preset${model.flat.length === 1 ? "" : "s"}`,
);

$effect(() => {
	if (!open) return;
	query = "";
	selectedIndex = 0;
	previewArmed = false;
	returnFocusTo = document.activeElement instanceof HTMLElement ? document.activeElement : null;
	void tick().then(() => inputRef?.focus());
});

// Keep the cursor in range as results change.
$effect(() => {
	if (selectedIndex >= model.flat.length) {
		selectedIndex = Math.max(0, model.flat.length - 1);
	}
});

// Armed only once the user moves or types, so opening the picker changes nothing. `untrack` because an effect that reads and writes the same state never settles.
$effect(() => {
	if (!open || !previewArmed) return;
	const preset = selectedPreset;
	if (preset) untrack(() => onpreview?.(preset));
});

// Restore covers every close path, including a shortcut toggling `open`; cancel() alone leaked the preview into the project.
let committed = false;
let wasOpen = false;
$effect(() => {
	if (open) {
		wasOpen = true;
		return;
	}
	if (!wasOpen) return;
	wasOpen = false;
	if (!committed) onrestore?.();
	committed = false;
	previewArmed = false;
});

function cancel() {
	onOpenChange(false);
	returnFocusTo?.focus();
}

function apply(p: Preset) {
	committed = true;
	onapply(p);
	onOpenChange(false);
	returnFocusTo?.focus();
}

function select(index: number) {
	previewArmed = true;
	selectedIndex = index;
	void tick().then(() => {
		listRef
			?.querySelector<HTMLElement>(`[data-preset-index="${selectedIndex}"]`)
			?.scrollIntoView({ block: "nearest" });
	});
}

// Jump a whole row, preserving the column (clamped on a shorter row); a no-op at the top and bottom edges.
function moveRow(dir: 1 | -1) {
	const next = rowMoveIndex(model, selectedIndex, dir);
	if (next !== null) select(next);
}

function moveCol(delta: 1 | -1) {
	select(clampIndex(selectedIndex + delta, model.flat.length));
}

// Left and Right belong to the search caret first, so only navigate at the matching edge or an empty field.
function caretAtStart(): boolean {
	if (!inputRef) return true;
	return inputRef.selectionStart === 0 && inputRef.selectionEnd === 0;
}
function caretAtEnd(): boolean {
	if (!inputRef) return true;
	const len = inputRef.value.length;
	return inputRef.selectionStart === len && inputRef.selectionEnd === len;
}

function clearQuery() {
	query = "";
	selectedIndex = 0;
	inputRef?.focus();
}

function handleKeydown(e: KeyboardEvent) {
	if (e.key === "Escape") {
		e.preventDefault();
		if (resolveEscape(query) === "clear") clearQuery();
		else cancel();
		return;
	}
	if (e.key === "ArrowDown") {
		e.preventDefault();
		moveRow(1);
		return;
	}
	if (e.key === "ArrowUp") {
		e.preventDefault();
		moveRow(-1);
		return;
	}
	if (e.key === "ArrowRight" && caretAtEnd()) {
		e.preventDefault();
		moveCol(1);
		return;
	}
	if (e.key === "ArrowLeft" && caretAtStart()) {
		e.preventDefault();
		moveCol(-1);
		return;
	}
	if (e.key === "Enter") {
		e.preventDefault();
		const p = model.flat[selectedIndex];
		if (p) apply(p);
	}
}

// Hover claims the cursor only after a real pointer move; otherwise a resting pointer yanked selection back on every arrow key.
let pointerActive = $state(false);
function hoverSelect(index: number) {
	if (!pointerActive) return;
	previewArmed = true;
	selectedIndex = index;
}

function categoryIcon(category: string): IconComponent {
	switch (category) {
		case "Current":
			return Star;
		case "Results":
			return Search;
		case "Studio":
			return Clapperboard;
		case "Instagram":
			return Camera;
		case "YouTube":
			return MonitorPlay;
		case "X / Twitter":
			return MessageCircle;
		case "TikTok":
			return Music2;
		case "LinkedIn":
			return Briefcase;
		default:
			return Sparkles;
	}
}

// Ancestors use transform or filter, which pins `position: fixed` to them; `inert` is also what makes aria-modal real, or Tab walks into the editor behind.
const INERT_FLAG = "data-preset-picker-inert";

function releaseInert(el: Element) {
	el.removeAttribute("inert");
	el.removeAttribute(INERT_FLAG);
}

function overlay(node: HTMLElement) {
	document.body.appendChild(node);
	// If this component dies mid-render, `destroy` never runs and the app stays unclickable; sweeping our marker makes that recoverable.
	for (const stale of Array.from(document.querySelectorAll(`[${INERT_FLAG}]`))) releaseInert(stale);
	const blocked = Array.from(document.body.children).filter((el) => !el.contains(node));
	for (const el of blocked) {
		el.setAttribute("inert", "");
		el.setAttribute(INERT_FLAG, "");
	}
	return {
		destroy() {
			for (const el of blocked) releaseInert(el);
			if (node.parentNode === document.body) {
				document.body.removeChild(node);
			}
		},
	};
}

// Svelte transitions are JS-driven, so the global prefers-reduced-motion CSS
// block never reaches them. Zero duration is the opt-out.
const motion = $derived(prefersReducedMotion.current ? 0 : 1);
</script>

<!-- Tiny WYSIWYG preview: real background (or wallpaper thumb) with the video
     frame inset by the preset's padding, at the correct aspect. The outer box
     is a fixed size so labels line up down the column whatever the aspect. -->
{#snippet thumb(preset: Preset)}
  {@const inset = frameInsetPct(preset.padding)}
  <div class="grid h-10 w-14 shrink-0 place-items-center">
    <div
      class={cn(
        "relative overflow-hidden rounded-md ring-1 ring-inset ring-border/40",
        aspectClass(preset.aspect),
        preset.aspect === "9:16" ? "h-full" : "w-full",
      )}
    >
      {#if preset.bg === "wallpaper" && preset.value}
        <LazyExternalImage
          assetId={wallpaperId(preset)}
          alt=""
          tier="thumb"
          class="absolute inset-0 size-full object-cover"
        />
      {:else}
        <div class="absolute inset-0" style={bgPreviewStyle(preset)}></div>
      {/if}
      <div
        class="absolute rounded-[2px] bg-background/80 ring-1 ring-inset ring-foreground/15"
        style="inset: {inset}%"
      ></div>
    </div>
  </div>
{/snippet}

{#if open}
  <div
    use:overlay
    class="fixed inset-0 z-100 flex items-start justify-center bg-background/55 px-4 pt-[10vh]"
    role="presentation"
    onpointerdown={(e) => {
      if (e.target === e.currentTarget) cancel();
    }}
    in:fade={{ duration: 140 * motion }}
    out:fade={{ duration: 100 * motion }}
  >
    <div
      bind:this={dialogRef}
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      aria-label="Choose a preset"
      onkeydown={handleKeydown}
      in:scale={{ duration: 180 * motion, start: 0.97, easing: cubicOut }}
      out:scale={{ duration: 120 * motion, start: 0.98 }}
      class="flex max-h-[78vh] w-full max-w-2xl flex-col overflow-hidden rounded-2xl border border-border/60 bg-popover shadow-2xl"
    >
      <div class="flex items-center gap-2 border-b border-border/60 px-3 py-2.5">
        <Search class="size-4 shrink-0 text-muted-foreground" />
        <input
          bind:this={inputRef}
          bind:value={query}
          oninput={() => {
            selectedIndex = 0;
            previewArmed = true;
          }}
          type="text"
          role="combobox"
          aria-expanded="true"
          aria-controls="{baseId}-listbox"
          aria-activedescendant={selectedPreset
            ? optionId(baseId, selectedIndex)
            : undefined}
          aria-label="Search presets"
          placeholder="Search presets, platforms, aspect ratios…"
          class="flex-1 bg-transparent text-[13px] text-foreground placeholder:text-muted-foreground/60 focus:outline-none"
          spellcheck="false"
          autocomplete="off"
        />
        {#if query}
          <button
            type="button"
            onclick={clearQuery}
            aria-label="Clear search"
            class="grid size-6 place-items-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
          >
            <X class="size-3.5" />
          </button>
        {/if}
      </div>

      <div class="sr-only" aria-live="polite">
        {resultSummary}{selectedPreset ? `, ${selectedPreset.label} selected` : ""}
      </div>

      <div
        bind:this={listRef}
        onpointermove={() => (pointerActive = true)}
        id="{baseId}-listbox"
        role="listbox"
        tabindex="-1"
        aria-label="Presets"
        class="flex flex-1 flex-col gap-2 overflow-y-auto px-2 py-2 scrollbar-transparent"
      >
        {#if model.flat.length === 0}
          <div class="px-3 py-10 text-center">
            <p class="text-[13px] text-foreground">No presets match “{query}”</p>
            <p class="mt-1 text-[11px] text-muted-foreground">
              Try a platform, a look, or an aspect like 9:16.
            </p>
          </div>
        {:else}
          {#each model.groups as group (group.category)}
            {@const CatIcon = categoryIcon(group.category)}
            <div class="flex flex-col gap-1">
              <div
                role="presentation"
                class="sticky top-0 z-10 flex items-center gap-1.5 bg-popover/95 px-2 py-1 text-[11px] font-semibold text-muted-foreground"
              >
                <CatIcon class="size-3 text-muted-foreground" />
                {group.category}
              </div>
              <div class="flex flex-col gap-1">
                {#each group.rows as row}
                  <div class={GRID_CLASS}>
                    {#each row as cell (cell.preset.id + ":" + cell.index)}
                      {@const preset = cell.preset}
                      {@const active = cell.index === selectedIndex}
                      {@const isApplied = preset.id === currentId}
                      <!-- A real button (native click + activation) but
                           tabindex -1: aria-activedescendant owns the cursor,
                           so the cells must not become a second tab ring. -->
                      <button
                        type="button"
                        tabindex="-1"
                        id={optionId(baseId, cell.index)}
                        role="option"
                        aria-selected={active}
                        data-preset-index={cell.index}
                        onpointerenter={() => hoverSelect(cell.index)}
                        onclick={() => apply(preset)}
                        class={cn(
                          "flex cursor-default items-center gap-2.5 rounded-lg border px-2 py-1.5 text-left transition-colors duration-150",
                          active
                            ? "border-border/60 bg-muted/60"
                            : "border-transparent hover:bg-muted/40",
                        )}
                      >
                        {@render thumb(preset)}
                        <div class="min-w-0 flex-1">
                          <div class="flex items-center gap-1.5">
                            <span class="truncate text-[13px] font-medium text-foreground">
                              {preset.label}
                            </span>
                            <span
                              class="inline-flex h-4 shrink-0 items-center rounded border border-border/40 bg-muted/40 px-1 font-mono text-[10px] text-muted-foreground"
                            >
                              {preset.aspect}
                            </span>
                            {#if isApplied}
                              <Check class="size-3 shrink-0 text-muted-foreground" />
                              <span class="sr-only">Currently applied</span>
                            {/if}
                          </div>
                          {#if preset.description}
                            <div class="truncate text-[11px] text-muted-foreground">
                              {preset.description}
                            </div>
                          {/if}
                        </div>
                        {#if active}
                          <CornerDownLeft class="size-3 shrink-0 text-muted-foreground" />
                        {/if}
                      </button>
                    {/each}
                  </div>
                {/each}
              </div>
            </div>
          {/each}
        {/if}
      </div>

      <div
        class="flex h-9 shrink-0 items-center justify-between gap-2 border-t border-border/60 bg-muted/30 px-3 text-[11px] text-muted-foreground"
      >
        <span class="inline-flex items-center gap-1.5">
          <KbdGroup>
            <Kbd>↑</Kbd>
            <Kbd>↓</Kbd>
          </KbdGroup>
          <span>Navigate</span>
        </span>
        <span class="inline-flex items-center gap-3">
          <span class="inline-flex items-center gap-1.5">
            <Kbd>Esc</Kbd>
            <span>{query ? "Clear" : "Cancel"}</span>
          </span>
          <span class="inline-flex items-center gap-1.5">
            <Kbd>↵</Kbd>
            <span>Apply</span>
          </span>
        </span>
      </div>
    </div>
  </div>
{/if}
