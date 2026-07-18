<script lang="ts" module>
  import type { BackgroundType } from "$lib/stores/editor-store.svelte";

  export type Preset = {
    id: string;
    label: string;
    category: string;
    aspect: string;
    description?: string;
    bg: BackgroundType;
    value?: string;
    padding: number;
    blur: number;
    layout?: "auto" | "crop";
    keywords?: string[];
  };

  export const PRESETS: Preset[] = [
    // Studio looks
    {
      id: "studio",
      label: "Studio",
      category: "Studio",
      aspect: "Source",
      description: "Soft blue gradient, centred padding",
      bg: "gradient",
      value: "linear-gradient(135deg, #0f172a 0%, #1d4ed8 100%)",
      padding: 3,
      blur: 18,
      layout: "auto",
      keywords: ["gradient", "blue"],
    },
    {
      id: "focus",
      label: "Focus",
      category: "Studio",
      aspect: "Source",
      description: "Deep navy, minimal frame",
      bg: "color",
      value: "#0b1120",
      padding: 2,
      blur: 0,
      layout: "auto",
      keywords: ["solid", "dark"],
    },
    {
      id: "spotlight",
      label: "Spotlight",
      category: "Studio",
      aspect: "Source",
      description: "Wallpaper backdrop with strong blur",
      bg: "wallpaper",
      value: "asset:wallpaper7",
      padding: 5,
      blur: 36,
      layout: "auto",
      keywords: ["wallpaper", "blur"],
    },
    {
      id: "edge",
      label: "Edge to Edge",
      category: "Studio",
      aspect: "Source",
      description: "No padding, full bleed",
      bg: "color",
      value: "#020617",
      padding: 0,
      blur: 0,
      layout: "crop",
      keywords: ["bleed", "full"],
    },

    // Instagram
    {
      id: "ig-post",
      label: "Post",
      category: "Instagram",
      aspect: "1:1",
      description: "Square feed post",
      bg: "gradient",
      value: "linear-gradient(135deg, #833ab4 0%, #fd1d1d 50%, #fcb045 100%)",
      padding: 6,
      blur: 24,
      layout: "auto",
      keywords: ["instagram", "ig", "square", "feed"],
    },
    {
      id: "ig-story",
      label: "Story",
      category: "Instagram",
      aspect: "9:16",
      description: "Vertical full-screen story",
      bg: "gradient",
      value: "linear-gradient(180deg, #1e1b4b 0%, #831843 100%)",
      padding: 8,
      blur: 28,
      layout: "auto",
      keywords: ["instagram", "ig", "vertical", "9:16"],
    },
    {
      id: "ig-reel",
      label: "Reel",
      category: "Instagram",
      aspect: "9:16",
      description: "Reels-friendly vertical frame",
      bg: "color",
      value: "#0a0a0a",
      padding: 4,
      blur: 0,
      layout: "auto",
      keywords: ["instagram", "ig", "reel", "vertical"],
    },
    {
      id: "ig-landscape",
      label: "Landscape",
      category: "Instagram",
      aspect: "1.91:1",
      description: "Wide feed post",
      bg: "gradient",
      value: "linear-gradient(135deg, #f472b6 0%, #8b5cf6 100%)",
      padding: 4,
      blur: 16,
      layout: "auto",
      keywords: ["instagram", "ig", "landscape", "wide"],
    },

    // YouTube
    {
      id: "yt-standard",
      label: "Standard",
      category: "YouTube",
      aspect: "16:9",
      description: "Classic 1080p / 4K landscape",
      bg: "color",
      value: "#0f0f0f",
      padding: 3,
      blur: 0,
      layout: "auto",
      keywords: ["youtube", "yt", "16:9", "landscape"],
    },
    {
      id: "yt-shorts",
      label: "Shorts",
      category: "YouTube",
      aspect: "9:16",
      description: "Vertical mobile-first",
      bg: "gradient",
      value: "linear-gradient(180deg, #7f1d1d 0%, #0f0f0f 100%)",
      padding: 6,
      blur: 20,
      layout: "auto",
      keywords: ["youtube", "yt", "shorts", "vertical"],
    },
    {
      id: "yt-thumbnail",
      label: "Thumbnail Safe",
      category: "YouTube",
      aspect: "16:9",
      description: "Extra padding for thumbnail safe area",
      bg: "gradient",
      value: "linear-gradient(135deg, #dc2626 0%, #1e293b 100%)",
      padding: 5,
      blur: 12,
      layout: "auto",
      keywords: ["youtube", "yt", "thumbnail"],
    },

    // X / Twitter
    {
      id: "x-post",
      label: "Post",
      category: "X / Twitter",
      aspect: "16:9",
      description: "Inline timeline video",
      bg: "color",
      value: "#15202b",
      padding: 3,
      blur: 0,
      layout: "auto",
      keywords: ["twitter", "x", "post", "16:9"],
    },
    {
      id: "x-square",
      label: "Square",
      category: "X / Twitter",
      aspect: "1:1",
      description: "Mobile-friendly square",
      bg: "gradient",
      value: "linear-gradient(135deg, #1d9bf0 0%, #0f172a 100%)",
      padding: 5,
      blur: 18,
      layout: "auto",
      keywords: ["twitter", "x", "square"],
    },
    {
      id: "x-vertical",
      label: "Vertical",
      category: "X / Twitter",
      aspect: "9:16",
      description: "Vertical autoplay clip",
      bg: "color",
      value: "#000000",
      padding: 4,
      blur: 0,
      layout: "auto",
      keywords: ["twitter", "x", "vertical"],
    },

    // TikTok
    {
      id: "tt-feed",
      label: "Feed",
      category: "TikTok",
      aspect: "9:16",
      description: "Standard For-You feed clip",
      bg: "gradient",
      value: "linear-gradient(180deg, #fe2c55 0%, #25f4ee 100%)",
      padding: 6,
      blur: 22,
      layout: "auto",
      keywords: ["tiktok", "tt", "vertical"],
    },

    // LinkedIn
    {
      id: "li-square",
      label: "Square",
      category: "LinkedIn",
      aspect: "1:1",
      description: "High-engagement square post",
      bg: "color",
      value: "#0a66c2",
      padding: 5,
      blur: 0,
      layout: "auto",
      keywords: ["linkedin", "li", "square"],
    },
    {
      id: "li-landscape",
      label: "Landscape",
      category: "LinkedIn",
      aspect: "1.91:1",
      description: "Native landscape feed video",
      bg: "gradient",
      value: "linear-gradient(135deg, #0a66c2 0%, #0f172a 100%)",
      padding: 4,
      blur: 14,
      layout: "auto",
      keywords: ["linkedin", "li", "landscape"],
    },
  ];
</script>

<script lang="ts">
  import LazyExternalImage from "$components/common/LazyExternalImage.svelte";
  import {
    aspectClass,
    bgPreviewStyle,
    buildModel,
    clampIndex,
    filterPresets,
    frameInsetPct,
    groupPresets,
    rowMoveIndex,
    wallpaperId,
  } from "./preset-picker.logic";
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
  } from "@recast/icons";
  import type { IconComponent } from "@recast/icons";
  import { Kbd, KbdGroup } from "@recast/ui/kbd";
  import { cn } from "@recast/ui/utils";
  import { tick } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { fade, scale } from "svelte/transition";

  interface Props {
    open: boolean;
    onOpenChange: (v: boolean) => void;
    onapply: (preset: Preset) => void;
    /** Id of the preset currently applied to the project, if any. */
    currentId?: string | null;
  }

  let { open, onOpenChange, onapply, currentId = null }: Props = $props();

  let query = $state("");
  let selectedIndex = $state(0);
  let inputRef = $state<HTMLInputElement | null>(null);
  let listRef = $state<HTMLDivElement | null>(null);

  const COLS = 2;

  const currentPreset = $derived(
    currentId ? (PRESETS.find((p) => p.id === currentId) ?? null) : null,
  );

  const filtered = $derived(filterPresets(PRESETS, query));
  const grouped = $derived(groupPresets(filtered, query, currentPreset));
  const model = $derived(buildModel(grouped, COLS));

  $effect(() => {
    if (open) {
      query = "";
      selectedIndex = 0;
      void tick().then(() => inputRef?.focus());
    }
  });

  // Keep the cursor in range as results change.
  $effect(() => {
    if (selectedIndex >= model.flat.length) {
      selectedIndex = Math.max(0, model.flat.length - 1);
    }
  });

  function close() {
    onOpenChange(false);
  }

  function apply(p: Preset) {
    onapply(p);
    close();
  }

  // Vertical move: jump a whole row, preserving the column (clamped when the
  // target row is shorter). See `rowMoveIndex`. No-op at the top/bottom edge.
  function moveRow(dir: 1 | -1) {
    const next = rowMoveIndex(model, selectedIndex, dir);
    if (next === null) return;
    selectedIndex = next;
    scrollSelectedIntoView();
  }

  function moveCol(delta: 1 | -1) {
    selectedIndex = clampIndex(selectedIndex + delta, model.flat.length);
    scrollSelectedIntoView();
  }

  // Left/Right belong to the search caret first; only navigate when the caret
  // sits at the matching edge (or the field is empty) so typing isn't hijacked.
  function caretAtStart(): boolean {
    if (!inputRef) return true;
    return inputRef.selectionStart === 0 && inputRef.selectionEnd === 0;
  }
  function caretAtEnd(): boolean {
    if (!inputRef) return true;
    const len = inputRef.value.length;
    return inputRef.selectionStart === len && inputRef.selectionEnd === len;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      close();
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

  function scrollSelectedIntoView() {
    void tick().then(() => {
      const el = listRef?.querySelector<HTMLElement>(
        `[data-preset-index="${selectedIndex}"]`,
      );
      el?.scrollIntoView({ block: "nearest" });
    });
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

  // Portal: ancestors of this component (e.g. the editor preview) use
  // `transform`/`filter`, which pins `position: fixed` to that ancestor and
  // breaks z-index relative to the rest of the app. Re-parenting to body
  // sidesteps that entirely.
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return {
      destroy() {
        if (node.parentNode === document.body) {
          document.body.removeChild(node);
        }
      },
    };
  }
</script>

<!-- Tiny WYSIWYG preview: real background (or wallpaper thumb) with the video
     frame inset by the preset's padding, at the correct aspect. -->
{#snippet thumb(preset: Preset)}
  {@const inset = frameInsetPct(preset.padding)}
  <div
    class={cn(
      "relative h-9 shrink-0 overflow-hidden rounded-md ring-1 ring-inset ring-border/40",
      aspectClass(preset.aspect),
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
{/snippet}

{#if open}
  <div
    use:portal
    class="fixed inset-0 z-100 flex items-start justify-center bg-background/60 px-4 pt-[12vh] backdrop-blur-sm"
    role="presentation"
    onpointerdown={(e) => {
      if (e.target === e.currentTarget) close();
    }}
    in:fade={{ duration: 140 }}
    out:fade={{ duration: 100 }}
  >
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Preset picker"
      onkeydown={handleKeydown}
      tabindex="-1"
      in:scale={{ duration: 180, start: 0.97, easing: cubicOut }}
      out:scale={{ duration: 120, start: 0.98 }}
      class="flex w-full max-w-2xl flex-col overflow-hidden rounded-2xl border border-border/60 bg-popover/95 shadow-2xl ring-1 ring-border/40 backdrop-blur-xl"
    >
      <!-- Search header -->
      <div class="flex items-center gap-2 border-b border-border/60 px-3 py-2.5">
        <Search class="size-4 shrink-0 text-muted-foreground" />
        <input
          bind:this={inputRef}
          bind:value={query}
          oninput={() => (selectedIndex = 0)}
          type="text"
          placeholder="Search presets, platforms, aspect ratios…"
          class="flex-1 bg-transparent text-[13px] text-foreground placeholder:text-muted-foreground/60 focus:outline-none"
          spellcheck="false"
          autocomplete="off"
        />
        <Kbd>Esc</Kbd>
      </div>

      <!-- Grouped list -->
      <div
        bind:this={listRef}
        class="flex max-h-[60vh] flex-col gap-2 overflow-y-auto px-2 py-2 scrollbar-transparent"
      >
        {#if model.flat.length === 0}
          <div class="px-3 py-10 text-center text-[12px] text-muted-foreground">
            No presets match "{query}"
          </div>
        {:else}
          {#each model.groups as group (group.category)}
            {@const CatIcon = categoryIcon(group.category)}
            <div class="flex flex-col gap-1">
              <div
                class="flex items-center gap-1.5 px-2 pt-1 text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
              >
                <CatIcon class="size-3 text-primary/70" />
                {group.category}
              </div>
              <div class="flex flex-col gap-1">
                {#each group.rows as row}
                  <div class="grid grid-cols-2 gap-1">
                    {#each row as cell (cell.preset.id + ":" + cell.index)}
                      {@const preset = cell.preset}
                      {@const active = cell.index === selectedIndex}
                      {@const isApplied = preset.id === currentId}
                      <button
                        type="button"
                        data-preset-index={cell.index}
                        onpointerenter={() => (selectedIndex = cell.index)}
                        onclick={() => apply(preset)}
                        class={cn(
                          "group relative flex items-center gap-2.5 rounded-lg border px-2 py-1.5 text-left transition-all duration-150",
                          active
                            ? "border-border/60 bg-muted/60 shadow-(--shadow-craft-inset)"
                            : "border-transparent hover:bg-muted/40",
                          isApplied && "ring-1 ring-primary/40",
                        )}
                      >
                        {@render thumb(preset)}
                        <div class="min-w-0 flex-1">
                          <div class="flex items-center gap-1.5">
                            <span
                              class="truncate text-[12px] font-semibold text-foreground"
                            >
                              {preset.label}
                            </span>
                            <span
                              class="inline-flex h-4 items-center rounded border border-border/40 bg-muted/40 px-1 font-mono text-[9px] font-semibold text-muted-foreground"
                            >
                              {preset.aspect}
                            </span>
                          </div>
                          {#if preset.description}
                            <div
                              class="truncate text-[10.5px] text-muted-foreground/80"
                            >
                              {preset.description}
                            </div>
                          {/if}
                        </div>
                        {#if active}
                          <CornerDownLeft
                            class="size-3 shrink-0 text-muted-foreground"
                          />
                        {:else if isApplied}
                          <Check class="size-3 shrink-0 text-primary" />
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

      <!-- Footer -->
      <div
        class="flex h-9 shrink-0 items-center justify-between gap-2 border-t border-border/60 bg-muted/30 px-3 text-[10.5px] text-muted-foreground"
      >
        <span class="inline-flex items-center gap-1.5">
          <KbdGroup>
            <Kbd>↑</Kbd>
            <Kbd>↓</Kbd>
            <Kbd>←</Kbd>
            <Kbd>→</Kbd>
          </KbdGroup>
          <span>Navigate</span>
        </span>
        <span class="inline-flex items-center gap-1.5">
          <Kbd>↵</Kbd>
          <span>Apply</span>
        </span>
      </div>
    </div>
  </div>
{/if}
