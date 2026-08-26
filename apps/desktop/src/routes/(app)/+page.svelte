<script lang="ts">
import { goto } from "$app/navigation";
import { AssetCard, LibraryEmpty, LibraryError, LibrarySkeletonGrid } from "$components/library";
import StudioPage from "$components/layout/StudioPage.svelte";
import { PlayerDialog } from "$components/recast";
import {
	getOutputDir,
	launchRecordingPanel,
	openCameraPreviewWindow,
	openFileLocation,
	type RecordingEntry,
} from "$lib/ipc";
import { cardShellClass, listClass } from "$lib/library/card-styles";
import { openInEditor as openEditorWindow } from "$lib/library/editor-window";
import { libraryStatus } from "$lib/library/status";
import { chordLabel } from "$lib/shortcuts/registry.svelte";
import { spawnOverlayWindow } from "$lib/windows/spawn-overlay";
import { getExtension } from "@recast/editor/lib/format/files";
import { motionDuration } from "@recast/editor/lib/motion.svelte";
import {
	AppWindow,
	Camera,
	ChevronDown,
	Crop,
	Download,
	Film,
	FolderOpen,
	Mic,
	Monitor,
	Plus,
	Radio,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import { ButtonGroup } from "@recast/ui/button-group";
import * as DropdownMenu from "@recast/ui/dropdown-menu";
import { Segmented } from "@recast/ui/segmented";
import { safeStorage } from "@recast/ui/persisted-state";
import { toast } from "@recast/ui/sonner";
import { listen } from "@tauri-apps/api/event";
import { onMount } from "svelte";
import { fade } from "svelte/transition";
import { createHomeState } from "./home.svelte";
import { greeting, type RecentItem } from "./home.logic";

const home = createHomeState();
const hello = greeting(new Date());
const recordShortcut = chordLabel("general.record");

let editorWindow = $state<"navigate" | "new-window">("navigate");
let playTarget = $state<RecordingEntry | null>(null);

const status = $derived(
	libraryStatus({
		loading: home.isLoading,
		error: home.loadError,
		total: home.hasAny ? home.recents.length : 0,
		matches: home.recents.length,
		query: "",
	}),
);

onMount(() => {
	home.fetchAll();
	editorWindow = safeStorage.get<"navigate" | "new-window">("recast-editor-window", editorWindow);
	const unlisten = listen("refresh-recordings", () => home.fetchAll());
	return () => unlisten.then((fn) => fn());
});

const modes = [
	{ label: "Screen", hint: "Full display", icon: Monitor, intent: "screen" },
	{ label: "Window", hint: "A single app", icon: AppWindow, intent: "window" },
	{ label: "Region", hint: "Drag an area", icon: Crop, intent: "region" },
	{ label: "Screen + camera", hint: "With webcam", icon: Camera, intent: "camera" },
] as const;

const filters = [
	{ value: "all", label: "All" },
	{ value: "recording", label: "Recordings" },
	{ value: "export", label: "Exports" },
];

function openItem(item: RecentItem) {
	if (item.kind === "recording") openEditorWindow(item.entry, editorWindow);
	else playTarget = item.entry;
}

async function showOutputFolder() {
	try {
		await openFileLocation(await getOutputDir());
	} catch (err) {
		toast.error(`Could not open folder: ${err}`);
	}
}

function openDevicePicker(type: "mic" | "camera") {
	return spawnOverlayWindow(`device-picker-${type}`, {
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
</script>

<StudioPage title={hello} subtitle="Pick up where you left off, or start something new.">
  {#snippet actions()}
    <ButtonGroup>
      <Button
        size="sm"
        class="gap-1.5"
        onclick={() => launchRecordingPanel()}
        title={`Start recording · ${recordShortcut}`}
      >
        <Radio class="size-4" />
        Record
      </Button>
      <DropdownMenu.Root>
        <DropdownMenu.Trigger>
          {#snippet child({ props })}
            <Button {...props as Record<string, unknown>} size="sm" class="px-2" aria-label="Recording options">
              <ChevronDown class="size-4" />
            </Button>
          {/snippet}
        </DropdownMenu.Trigger>
        <DropdownMenu.Content align="end" class="w-56">
          {#each modes as mode (mode.intent)}
            {@const Icon = mode.icon}
            <DropdownMenu.Item onSelect={() => launchRecordingPanel(mode.intent)}>
              <Icon class="size-3.5" />
              <span class="flex-1">{mode.label}</span>
              <span class="text-[10px] text-muted-foreground">{mode.hint}</span>
            </DropdownMenu.Item>
          {/each}
          <DropdownMenu.Separator />
          <DropdownMenu.Item onSelect={() => openCameraPreviewWindow()}>
            <Camera class="size-3.5" /> Camera preview
          </DropdownMenu.Item>
          <DropdownMenu.Item onSelect={() => openDevicePicker("mic")}>
            <Mic class="size-3.5" /> Pick microphone
          </DropdownMenu.Item>
          <DropdownMenu.Item onSelect={() => openDevicePicker("camera")}>
            <Camera class="size-3.5" /> Pick camera
          </DropdownMenu.Item>
          <DropdownMenu.Separator />
          <DropdownMenu.Item onSelect={showOutputFolder}>
            <FolderOpen class="size-3.5" /> Show recordings folder
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Root>
    </ButtonGroup>
  {/snippet}

  <div class="mx-auto flex max-w-6xl flex-col gap-4">
    <div class="flex items-center justify-between gap-2">
      <Segmented
        options={filters}
        value={home.filter}
        onValueChange={(v) => (home.filter = v as typeof home.filter)}
        fill={false}
        aria-label="Filter recent work"
      />
      <Button
        variant="ghost"
        size="xs"
        class="text-muted-foreground hover:text-foreground"
        onclick={() => goto(home.filter === "export" ? "/exports" : "/recasts")}
      >
        Open library
      </Button>
    </div>

    {#if status === "loading"}
      <LibrarySkeletonGrid view="grid" />
    {:else if status === "error"}
      <LibraryError
        title="Couldn't load your activity"
        message={home.loadError ?? "Unknown error"}
        onRetry={home.fetchAll}
      />
    {:else if status === "empty"}
      <LibraryEmpty
        icon={Film}
        title="Nothing here yet"
        description="Record your screen and your clips and exports land here, ready to edit."
      >
        {#snippet action()}
          <Button class="gap-2" onclick={() => launchRecordingPanel()}>
            <Radio class="size-4" /> Start recording
          </Button>
        {/snippet}
      </LibraryEmpty>
    {:else}
      <div class={listClass("grid")}>
        {#each home.recents as item, i (item.entry.path)}
          <div
            in:fade={{ duration: motionDuration(220), delay: motionDuration(Math.min(i * 25, 200)) }}
            title={item.entry.filename}
            class={cardShellClass("grid", false)}
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
        <button
          type="button"
          onclick={() => launchRecordingPanel()}
          class="group/new flex aspect-video flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-border/60 bg-card/30 text-muted-foreground transition-[background-color,border-color,color,transform] duration-150 ease-out hover:border-border hover:bg-card/60 hover:text-foreground focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/50 motion-safe:active:scale-[0.98]"
        >
          <span
            class="flex size-9 items-center justify-center rounded-full bg-foreground/5 transition-colors group-hover/new:bg-primary/10 group-hover/new:text-primary"
          >
            <Plus class="size-4" />
          </span>
          <span class="text-[12px] font-medium">New recording</span>
        </button>
      </div>
    {/if}
  </div>
</StudioPage>

{#if playTarget}
  <PlayerDialog entry={playTarget} onclose={() => (playTarget = null)} />
{/if}
