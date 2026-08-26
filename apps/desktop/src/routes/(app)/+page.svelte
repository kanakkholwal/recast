<script lang="ts">
import { goto } from "$app/navigation";
import { AssetCard } from "$components/library";
import NotchedShelf from "$components/layout/NotchedShelf.svelte";
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
	delay: motionDuration(80 + i * 45),
	start: 0.97,
	opacity: 0,
	easing: cubicOut,
});

function openItem(item: RecentItem) {
	if (item.kind === "recording") openEditorWindow(item.entry, editorWindow);
	else playTarget = item.entry;
}
</script>

<StudioPage title={hello} subtitle="Start a recording, or jump back into recent work.">
  <div class="relative mx-auto flex w-full max-w-3xl flex-col gap-10 pb-6 pt-2">
    <div
      aria-hidden="true"
      class="bg-grid-pattern pointer-events-none absolute -inset-x-8 -top-6 -z-10 h-72"
    ></div>

    <!-- Capture: one grouped card, headed by a notched shelf -->
    <section class="flex flex-col items-center">
      <NotchedShelf fill="text-card" class="h-9">
        <span class="px-2 text-[10.5px] font-semibold uppercase tracking-[0.14em] text-muted-foreground">
          Start a recording
        </span>
      </NotchedShelf>
      <div class="-mt-px w-full rounded-2xl rounded-t-none bg-card shadow-craft-floating ring-1 ring-inset ring-border/40">
        <div class="grid grid-cols-2 gap-1 p-2 sm:grid-cols-4">
          {#each modes as mode, i (mode.intent)}
            {@const Icon = mode.icon}
            <button
              type="button"
              onclick={() => launchRecordingPanel(mode.intent)}
              in:scale={rise(i)}
              class="group/mode relative flex flex-col gap-2.5 rounded-xl p-3.5 text-left transition-colors duration-150 hover:bg-foreground/[0.035] focus:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/50"
            >
              <span
                class="flex size-9 items-center justify-center rounded-lg bg-foreground/5 text-foreground transition-transform duration-200 ease-out motion-safe:group-hover/mode:scale-110"
              >
                <Icon class="size-5" />
              </span>
              <span class="flex flex-col gap-0.5">
                <span class="text-[12.5px] font-semibold text-foreground">{mode.label}</span>
                <span class="text-[11px] leading-snug text-muted-foreground">{mode.hint}</span>
              </span>
              <ArrowUpRight
                class="absolute right-3 top-3 size-3.5 text-muted-foreground opacity-0 transition-all duration-200 ease-out motion-safe:group-hover/mode:-translate-y-0.5 motion-safe:group-hover/mode:translate-x-0.5 group-hover/mode:opacity-100"
              />
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
        <div
          class="-mx-1 flex gap-3 overflow-x-auto px-1 pb-1 no-scrollbar [mask-image:linear-gradient(to_right,black_calc(100%-2.5rem),transparent)]"
        >
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
