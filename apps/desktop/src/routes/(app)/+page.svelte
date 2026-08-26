<script lang="ts">
import { goto } from "$app/navigation";
import { AssetCard } from "$components/library";
import StudioPage from "$components/layout/StudioPage.svelte";
import { PlayerDialog } from "$components/recast";
import { launchRecordingPanel, type RecordingEntry } from "$lib/ipc";
import { cardShellClass } from "$lib/library/card-styles";
import { openInEditor as openEditorWindow } from "$lib/library/editor-window";
import { getExtension } from "@recast/editor/lib/format/files";
import { motionDuration } from "@recast/editor/lib/motion.svelte";
import {
	AppWindow,
	ArrowRight,
	ArrowUpRight,
	Camera,
	Crop,
	Download,
	Film,
	Monitor,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import { Cutout } from "@recast/ui/cutout";
import { safeStorage } from "@recast/ui/persisted-state";
import { listen } from "@tauri-apps/api/event";
import { onMount } from "svelte";
import { cubicOut } from "svelte/easing";
import { fade, scale } from "svelte/transition";
import { greeting, type RecentItem } from "./home.logic";
import { createHomeState } from "./home.svelte";

const home = createHomeState();
const hello = greeting(new Date());
let editorWindow = $state<"navigate" | "new-window">("navigate");
let playTarget = $state<RecordingEntry | null>(null);

onMount(() => {
	home.fetchAll();
	editorWindow = safeStorage.get<"navigate" | "new-window">("recast-editor-window", editorWindow);
	const unlisten = listen("refresh-recordings", () => home.fetchAll());
	return () => unlisten.then((fn) => fn());
});

const modes = [
	{ label: "Screen", hint: "Record a full display", icon: Monitor, intent: "screen" },
	{ label: "Window", hint: "Record one app window", icon: AppWindow, intent: "window" },
	{ label: "Region", hint: "Drag to select an area", icon: Crop, intent: "region" },
	{ label: "Screen + Camera", hint: "Screen with webcam overlay", icon: Camera, intent: "camera" },
] as const;

const rise = (i: number) => ({
	duration: motionDuration(300),
	delay: motionDuration(60 + i * 45),
	start: 0.96,
	opacity: 0,
	easing: cubicOut,
});

function openItem(item: RecentItem) {
	if (item.kind === "recording") openEditorWindow(item.entry, editorWindow);
	else playTarget = item.entry;
}
</script>

<StudioPage title={hello} subtitle="Start a recording, or jump back into recent work.">
  <div class="mx-auto flex w-full max-w-3xl flex-col gap-9 py-1">
    <section class="flex flex-col gap-3">
      <h2 class="px-0.5 text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-foreground/70">
        Start a recording
      </h2>
      <div class="relative isolate">
        <div
          aria-hidden="true"
          class="breathe pointer-events-none absolute -inset-x-6 -top-8 -z-10 h-36 rounded-full bg-foreground/[0.04] blur-3xl"
        ></div>
        <div class="grid grid-cols-2 gap-3 sm:grid-cols-4">
          {#each modes as mode, i (mode.intent)}
            {@const Icon = mode.icon}
            <button
              type="button"
              onclick={() => launchRecordingPanel(mode.intent)}
              in:scale={rise(i)}
              class="group/mode relative flex flex-col items-start gap-3 rounded-xl border border-border/60 bg-card p-4 text-left shadow-(--shadow-craft-inset) transition-[transform,border-color,box-shadow] duration-200 ease-out hover:border-border hover:shadow-craft-md focus:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/50 motion-safe:hover:-translate-y-1 motion-safe:active:scale-[0.98]"
            >
              <span
                class="flex size-10 items-center justify-center rounded-lg bg-foreground/5 text-foreground transition-transform duration-200 ease-out motion-safe:group-hover/mode:scale-110"
              >
                <Icon class="size-5" />
              </span>
              <span class="flex min-w-0 flex-col gap-0.5 pr-4">
                <span class="text-[13px] font-semibold text-foreground">{mode.label}</span>
                <span class="text-[11px] leading-snug text-muted-foreground">{mode.hint}</span>
              </span>

              <Cutout corner="tr" surface="background" radius={14} class="flex items-start justify-end pb-4 pl-4 pr-1.5 pt-1.5">
                <span
                  class="flex size-5 items-center justify-center rounded-full bg-foreground/5 text-muted-foreground/70 transition-all duration-200 ease-out group-hover/mode:bg-foreground/10 group-hover/mode:text-foreground motion-safe:group-hover/mode:-translate-y-0.5 motion-safe:group-hover/mode:translate-x-0.5"
                >
                  <ArrowUpRight class="size-3" />
                </span>
              </Cutout>
            </button>
          {/each}
        </div>
      </div>
    </section>

    {#if home.recents.length > 0}
      <section class="flex flex-col gap-3">
        <div class="flex items-center justify-between px-0.5">
          <h2 class="text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-foreground/70">
            Recent
          </h2>
          <Button
            variant="ghost"
            size="xs"
            class="gap-1 text-muted-foreground hover:text-foreground"
            onclick={() => goto("/recasts")}
          >
            Open library <ArrowRight class="size-3" />
          </Button>
        </div>
        <div class="-mx-1 flex gap-3 overflow-x-auto px-1 pb-1 no-scrollbar">
          {#each home.recents as item, i (item.entry.path)}
            <div
              in:fade={{ duration: motionDuration(220), delay: motionDuration(Math.min(i * 25, 200)) }}
              title={item.entry.filename}
              class="w-44 shrink-0 {cardShellClass('grid', false)}"
            >
              <AssetCard
                entry={item.entry}
                thumbnail={home.thumbnails[item.entry.path]}
                view="grid"
                placeholderIcon={item.kind === "export" ? Download : Film}
                typeLabel={getExtension(item.entry.filename).toUpperCase()}
                onOpen={() => openItem(item)}
              />
            </div>
          {/each}
        </div>
      </section>
    {/if}
  </div>
</StudioPage>

{#if playTarget}
  <PlayerDialog entry={playTarget} onclose={() => (playTarget = null)} />
{/if}

<style>
  .breathe {
    animation: breathe 6s ease-in-out infinite;
  }
  @keyframes breathe {
    0%,
    100% {
      opacity: 0.45;
      transform: scale(1);
    }
    50% {
      opacity: 0.9;
      transform: scale(1.05);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .breathe {
      animation: none;
    }
  }
</style>
