<script lang="ts">
import { goto } from "$app/navigation";
import {
	getOutputDir,
	launchRecordingPanel,
	listExports,
	listRecasts,
	openCameraPreviewWindow,
	openFileLocation,
	type RecordingEntry,
} from "$lib/ipc";
import { commandPalette } from "$lib/stores/command-palette.svelte";
import { chordLabel } from "$lib/shortcuts/registry.svelte";
import { formatSize, relativeDate } from "$lib/format/files";
import { openInEditor as openEditorWindow } from "$lib/library/editor-window";
import { recentSix } from "$lib/library/list";
import { PlayerDialog } from "$components/recast";
import { LibraryError } from "$components/library";
import { createThumbnailLoader } from "$lib/library/thumbnails";
import { spawnOverlayWindow } from "$lib/windows/spawn-overlay";
import { motionDuration } from "$lib/motion.svelte";
import Logo from "$components/logo.svelte";
import {
	AppWindow,
	ArrowRight,
	Camera,
	Crop,
	Download,
	Film,
	FolderOpen,
	Mic,
	Monitor,
	Radio,
	Search,
} from "@recast/icons";
import type { IconComponent } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { Kbd } from "@recast/ui/kbd";
import { Skeleton } from "@recast/ui/skeleton";
import { toast } from "@recast/ui/sonner";
import { cn } from "@recast/ui/utils";
import { safeStorage } from "@recast/ui/persisted-state";
import { listen } from "@tauri-apps/api/event";
import { onMount } from "svelte";
import { cubicOut } from "svelte/easing";
import { fade, fly } from "svelte/transition";

let recasts = $state<RecordingEntry[]>([]);
let exports_ = $state<RecordingEntry[]>([]);
let isLoading = $state(true);
/** Last scan failure, so a broken load can't read as an empty library. */
let loadError = $state<string | null>(null);
/** Export being previewed, matching how /exports opens one. */
let playTarget = $state<RecordingEntry | null>(null);
let thumbnails = $state<Record<string, string>>({});
let editorWindow = $state<"navigate" | "new-window">("navigate");
let now = $state(Date.now());
const loadThumbnails = createThumbnailLoader();

// Derived from the registry so it always matches the real binding (Mod+Alt+R)
// and is platform-correct, instead of a hardcoded chord that can drift.
const recordShortcut = chordLabel("general.record");
const paletteShortcut = chordLabel("general.palette");

// Single staggered entrance so the page reveals as one smooth cascade on first
// load. `rise(delay)` feeds `in:fly`. Durations go through `motionDuration`,
// which is reactive and already explains why WAAPI transitions have to be
// zeroed in JS rather than by the CSS media query.
const rise = (delay = 0) => ({
	y: 12,
	duration: motionDuration(340),
	delay: motionDuration(delay),
	easing: cubicOut,
});

onMount(() => {
	fetchAll();
	editorWindow = safeStorage.get<"navigate" | "new-window">("recast-editor-window", editorWindow);
	const unlisten = listen("refresh-recordings", () => fetchAll());
	const tick = window.setInterval(() => (now = Date.now()), 60_000);
	return () => {
		unlisten.then((fn) => fn());
		window.clearInterval(tick);
	};
});

async function fetchAll() {
	isLoading = true;
	try {
		const [r, e] = await Promise.all([listRecasts(), listExports()]);
		recasts = recentSix(r);
		exports_ = recentSix(e);
		loadError = null;
		void refreshThumbnails([...recasts, ...exports_]);
	} catch (err) {
		loadError = String(err);
		toast.error(`Could not load activity: ${err}`);
	} finally {
		isLoading = false;
	}
}

async function refreshThumbnails(items: RecordingEntry[]) {
	const next = await loadThumbnails(items);
	if (next) thumbnails = next;
}

// Closes over `now` so the relative label re-renders as the clock ticks.
const formatDate = (unix: number) => relativeDate(unix, { now });

const openInEditor = (entry: RecordingEntry) => openEditorWindow(entry, editorWindow);

async function showOutputFolder() {
	try {
		const dir = await getOutputDir();
		await openFileLocation(dir);
	} catch (err) {
		toast.error(`Could not open folder: ${err}`);
	}
}

async function openDevicePickerWindow(type: "mic" | "camera") {
	await spawnOverlayWindow(`device-picker-${type}`, {
		url: `/device-picker?type=${type}`,
		title: `Select ${type === "mic" ? "Microphone" : "Camera"}`,
		width: 320,
		height: 340,
		center: true,
		decorations: false,
		transparent: true,
		shadow: false,
		resizable: false,
	});
}

// Each tile opens the recorder preset to that capture intent (the panel picks
// it up on launch). "Screen + webcam" is honest: there's no webcam-only
// source, so it captures the screen with the camera overlay on.
const modes = [
	{
		id: "screen",
		label: "Full Screen",
		hint: "Capture an entire display",
		icon: Monitor,
		intent: "screen",
	},
	{
		id: "window",
		label: "Window",
		hint: "Capture a single app window",
		icon: AppWindow,
		intent: "window",
	},
	{
		id: "region",
		label: "Region",
		hint: "Drag to select an area",
		icon: Crop,
		intent: "region",
	},
	{
		id: "camera",
		label: "Screen + webcam",
		hint: "Add your webcam overlay",
		icon: Camera,
		intent: "camera",
	},
] as const;

type QuickAction = {
	id: string;
	label: string;
	icon: IconComponent;
	onClick: () => void;
};
const quickActions: QuickAction[] = [
	{
		id: "preview",
		label: "Camera preview",
		icon: Camera,
		onClick: () => openCameraPreviewWindow(),
	},
	{
		id: "mic",
		label: "Pick microphone",
		icon: Mic,
		onClick: () => openDevicePickerWindow("mic"),
	},
	{
		id: "cam",
		label: "Pick camera",
		icon: Camera,
		onClick: () => openDevicePickerWindow("camera"),
	},
	{
		id: "folder",
		label: "Show folder",
		icon: FolderOpen,
		onClick: () => showOutputFolder(),
	},
];
</script>

<div class="h-full overflow-y-auto scrollbar-transparent no-scrollbar">
  <div class="mx-auto flex max-w-3xl flex-col gap-10 px-6 py-12 md:py-16">
    <!-- Hero -->
    <header
      in:fly={rise(0)}
      class="flex flex-col items-center gap-3 text-center"
    >
      <span
        class="inline-flex items-center gap-1.5 rounded-full border border-border/50 bg-card/60 py-1 pl-1.5 pr-2.5 text-[10px] font-semibold uppercase tracking-[0.15em] text-foreground/70 backdrop-blur"
      >
        <Logo size="14" class="shrink-0 rounded-full" />
        Recast
      </span>
      <h1
        class="text-balance text-[34px] font-semibold leading-tight tracking-tight text-foreground md:text-[40px]"
      >
        <span class="bg-linear-to-r from-foreground to-foreground/55 bg-clip-text text-transparent">
          What do you want to capture?
        </span>
      </h1>
      <p class="max-w-md text-[13px] leading-relaxed text-muted-foreground">
        Pick a mode below or jump into the panel. Press
        <Kbd class="mx-0.5 align-middle">{paletteShortcut}</Kbd>
        anywhere to search every action.
      </p>
    </header>

    <!-- Search bar (opens command palette) -->
    <button
      type="button"
      onclick={() => commandPalette.show()}
      in:fly={rise(70)}
      class="group/search flex h-12 items-center gap-3 rounded-xl border border-border/60 bg-card/70 px-4 text-left shadow-(--shadow-craft-inset) backdrop-blur transition-all duration-200 hover:border-border hover:bg-card hover:shadow-craft-sm focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
    >
      <Search
        class="size-4 shrink-0 text-muted-foreground/70 transition-colors group-hover/search:text-foreground"
      />
      <span class="flex-1 text-[13px] font-medium text-muted-foreground/80">
        Search actions, recordings, exports…
      </span>
      <!-- The one Kbd that earns its place: this is a search field, where the
           chord is the affordance, not a button competing with its own label. -->
      <Kbd>{paletteShortcut}</Kbd>
    </button>

    <!-- Recording modes -->
    <section in:fly={rise(140)} class="flex flex-col gap-3">
      <div class="flex items-baseline justify-between px-1">
        <h2 class="text-[11px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70">
          Start a recording
        </h2>
        <Button
          variant="ghost"
          size="xs"
          class="h-7 gap-1 text-[11px] text-muted-foreground hover:text-foreground"
          onclick={() => launchRecordingPanel()}
          title={`Open the recorder  ·  ${recordShortcut}`}
        >
          Open panel
        </Button>
      </div>
      <div class="grid grid-cols-2 gap-2 sm:grid-cols-4">
        {#each modes as mode (mode.id)}
          {@const Icon = mode.icon}
          <button
            type="button"
            onclick={() => launchRecordingPanel(mode.intent)}
            class={cn(
              "group/mode relative flex aspect-[5/4] flex-col items-start justify-between overflow-hidden rounded-xl border border-border/60 bg-card/70 p-3 text-left shadow-(--shadow-craft-inset) backdrop-blur",
              "transition-all duration-200 hover:border-border hover:shadow-craft-sm motion-safe:hover:-translate-y-0.5",
              "focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/50",
            )}
          >
            <span
              class="flex size-8 items-center justify-center rounded-lg bg-foreground/5 text-foreground transition-colors group-hover/mode:bg-primary/10 group-hover/mode:text-primary"
            >
              <Icon class="size-4" />
            </span>
            <div class="flex w-full items-end justify-between gap-2">
              <div class="min-w-0">
                <div class="truncate text-[12.5px] font-semibold text-foreground">
                  {mode.label}
                </div>
                <div class="truncate text-[10.5px] text-muted-foreground/80">
                  {mode.hint}
                </div>
              </div>
              <ArrowRight
                class="size-3.5 shrink-0 text-muted-foreground/50 transition-all duration-200 group-hover/mode:translate-x-0.5 group-hover/mode:text-foreground"
              />
            </div>
          </button>
        {/each}
      </div>
    </section>

    <!-- Primary CTA + quick action chips -->
    <section in:fly={rise(210)} class="flex flex-col gap-3">
      <Button
        onclick={() => launchRecordingPanel()}
        size="lg"
        class="group/cta h-12 w-full gap-2 rounded-xl text-[13px] font-semibold"
      >
        <Radio class="size-4 transition-transform duration-200 motion-safe:group-hover/cta:rotate-12" />
        Launch recording panel
      </Button>
      <div class="flex flex-wrap gap-2">
        {#each quickActions as qa (qa.id)}
          {@const Icon = qa.icon}
          <button
            type="button"
            onclick={qa.onClick}
            class="inline-flex h-8 items-center gap-1.5 rounded-full border border-border/50 bg-card/60 px-3 text-[11.5px] font-medium text-muted-foreground transition-all duration-200 hover:-translate-y-px hover:border-border hover:bg-card hover:text-foreground hover:shadow-craft-sm focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
          >
            <Icon class="size-3.5" />
            {qa.label}
          </button>
        {/each}
      </div>
    </section>

    <!-- Recent strips -->
    {#if recasts.length > 0 || isLoading}
      <section in:fly={rise(280)} class="flex flex-col gap-3">
        <div class="flex items-baseline justify-between px-1">
          <h2 class="text-[11px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70">
            Recent recordings
          </h2>
          <Button
            variant="ghost"
            size="xs"
            class="h-7 gap-1 text-[11px] text-muted-foreground hover:text-foreground"
            onclick={() => goto("/recasts")}
          >
            See all
            <ArrowRight class="size-3" />
          </Button>
        </div>
        <div
          class="-mx-1 flex gap-2 overflow-x-auto px-1 pb-1 scrollbar-transparent"
        >
          {#if isLoading && recasts.length === 0}
            {#each { length: 4 } as _, i (i)}
              <Skeleton class="aspect-video w-44 shrink-0 rounded-lg" style="animation-delay: {i * 100}ms" />
            {/each}
          {:else}
            {#each recasts as entry, i (entry.path)}
              <button
                type="button"
                onclick={() => openInEditor(entry)}
                in:fade={{ duration: motionDuration(220), delay: motionDuration(i * 40) }}
                class="group/card flex w-44 shrink-0 flex-col gap-1.5 rounded-lg p-1 text-left transition-all duration-200 hover:bg-card/60 focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
              >
                <div
                  class="relative aspect-video overflow-hidden rounded-md border border-border/40 bg-muted/40 shadow-(--shadow-craft-inset) transition-transform duration-200 group-hover/card:shadow-craft-sm motion-safe:group-hover/card:-translate-y-0.5"
                >
                  {#if thumbnails[entry.path]}
                    <img
                      src={thumbnails[entry.path]}
                      alt=""
                      class="h-full w-full object-cover"
                    />
                  {:else}
                    <div class="grid h-full w-full place-items-center text-muted-foreground/50">
                      <Film class="size-5" />
                    </div>
                  {/if}
                </div>
                <div class="px-1">
                  <div class="truncate text-[11.5px] font-medium text-foreground">
                    {entry.filename}
                  </div>
                  <div class="truncate text-[10px] text-muted-foreground/70">
                    {formatSize(entry.sizeBytes)} · {formatDate(entry.created)}
                  </div>
                </div>
              </button>
            {/each}
          {/if}
        </div>
      </section>
    {/if}

    {#if exports_.length > 0 || isLoading}
      <section in:fly={rise(350)} class="flex flex-col gap-3">
        <div class="flex items-baseline justify-between px-1">
          <h2 class="text-[11px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70">
            Recent exports
          </h2>
          <Button
            variant="ghost"
            size="xs"
            class="h-7 gap-1 text-[11px] text-muted-foreground hover:text-foreground"
            onclick={() => goto("/exports")}
          >
            See all
            <ArrowRight class="size-3" />
          </Button>
        </div>
        <div
          class="-mx-1 flex gap-2 overflow-x-auto px-1 pb-1 scrollbar-transparent"
        >
          {#if isLoading && exports_.length === 0}
            {#each { length: 4 } as _, i (i)}
              <Skeleton class="aspect-video w-44 shrink-0 rounded-lg" style="animation-delay: {i * 100}ms" />
            {/each}
          {:else}
          {#each exports_ as entry, i (entry.path)}
            <button
              type="button"
              onclick={() => (playTarget = entry)}
              in:fade={{ duration: motionDuration(220), delay: motionDuration(i * 40) }}
              class="group/card flex w-44 shrink-0 flex-col gap-1.5 rounded-lg p-1 text-left transition-all duration-200 hover:bg-card/60 focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
            >
              <div
                class="relative aspect-video overflow-hidden rounded-md border border-border/40 bg-muted/40 shadow-(--shadow-craft-inset) transition-transform duration-200 group-hover/card:-translate-y-0.5 group-hover/card:shadow-craft-sm"
              >
                {#if thumbnails[entry.path]}
                  <img
                    src={thumbnails[entry.path]}
                    alt=""
                    class="h-full w-full object-cover"
                  />
                {:else}
                  <div class="grid h-full w-full place-items-center text-muted-foreground/50">
                    <Download class="size-5" />
                  </div>
                {/if}
                <span
                  class="absolute right-1 top-1 rounded-sm bg-background/85 px-1 py-px text-[8.5px] font-bold uppercase tracking-wider text-foreground/80 backdrop-blur"
                >
                  {entry.filename.split(".").pop() ?? ""}
                </span>
              </div>
              <div class="px-1">
                <div class="truncate text-[11.5px] font-medium text-foreground">
                  {entry.filename}
                </div>
                <div class="truncate text-[10px] text-muted-foreground/70">
                  {formatSize(entry.sizeBytes)} · {formatDate(entry.created)}
                </div>
              </div>
            </button>
          {/each}
          {/if}
        </div>
      </section>
    {/if}

    {#if loadError && recasts.length === 0 && exports_.length === 0}
      <div in:fly={rise(280)}>
        <LibraryError
          title="Couldn't load your activity"
          message={loadError}
          onRetry={fetchAll}
        />
      </div>
    {:else if !isLoading && recasts.length === 0 && exports_.length === 0}
      <div
        in:fly={rise(280)}
        class="flex flex-col items-center gap-3 rounded-xl border border-dashed border-border/60 bg-card/40 p-8 text-center"
      >
        <div
          class="flex size-10 items-center justify-center rounded-xl bg-foreground/5 text-foreground"
        >
          <Film class="size-5" />
        </div>
        <div>
          <p class="text-[13px] font-semibold text-foreground">
            No recordings yet
          </p>
          <p class="mt-1 text-[11px] text-muted-foreground">
            Start a recording and your clips and exports land here.
          </p>
        </div>
      </div>
    {/if}
  </div>
</div>

{#if playTarget}
  <PlayerDialog entry={playTarget} onclose={() => (playTarget = null)} />
{/if}
