<script lang="ts">
import StudioPage from "$components/layout/StudioPage.svelte";
import { launchRecordingPanel } from "$lib/ipc";
import { chordLabel } from "$lib/shortcuts/registry.svelte";
import { AppWindow, ArrowUpRight, Camera, Crop, Monitor } from "@recast/icons";
import { greeting } from "./home.logic";

const hello = greeting(new Date());
const recordShortcut = chordLabel("general.record");

const modes = [
	{ label: "Screen", hint: "Record a full display", icon: Monitor, intent: "screen" },
	{ label: "Window", hint: "Record one app window", icon: AppWindow, intent: "window" },
	{ label: "Region", hint: "Drag to select an area", icon: Crop, intent: "region" },
	{
		label: "Screen + Camera",
		hint: "Screen with a webcam overlay",
		icon: Camera,
		intent: "camera",
	},
] as const;
</script>

<StudioPage title={hello} subtitle="What would you like to capture?">
  <div class="relative mx-auto flex min-h-full w-full max-w-2xl flex-col justify-center gap-6 py-10">
    <div
      aria-hidden="true"
      class="bg-grid-pattern pointer-events-none absolute -inset-x-10 -top-10 -z-10 h-96"
    ></div>

    <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
      {#each modes as mode (mode.intent)}
        {@const Icon = mode.icon}
        <button
          type="button"
          onclick={() => launchRecordingPanel(mode.intent)}
          class="group/mode relative flex flex-col gap-4 overflow-hidden rounded-2xl border border-border/60 bg-card p-5 text-left shadow-(--shadow-craft-inset) transition-[transform,border-color,box-shadow] duration-200 ease-out hover:border-border hover:shadow-craft-md focus:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/50 motion-safe:hover:-translate-y-1 motion-safe:active:scale-[0.99]"
        >
          <span
            class="flex size-12 items-center justify-center rounded-xl bg-foreground/5 text-foreground transition-transform duration-200 ease-out motion-safe:group-hover/mode:scale-110"
          >
            <Icon class="size-6" />
          </span>
          <span class="flex flex-col gap-1">
            <span class="text-[15px] font-semibold text-foreground">{mode.label}</span>
            <span class="text-[12px] leading-snug text-muted-foreground">{mode.hint}</span>
          </span>
          <ArrowUpRight
            class="absolute right-4 top-4 size-4 text-muted-foreground/40 opacity-0 transition-all duration-200 ease-out group-hover/mode:text-foreground group-hover/mode:opacity-100 motion-safe:group-hover/mode:-translate-y-0.5 motion-safe:group-hover/mode:translate-x-0.5"
          />
        </button>
      {/each}
    </div>

    <p class="text-center text-[11.5px] text-muted-foreground/70">
      Pick a mode to open the recorder, or press
      <kbd class="rounded border border-border/60 bg-card px-1 py-0.5 font-mono text-[10px] text-foreground/80">{recordShortcut}</kbd>
    </p>
  </div>
</StudioPage>
