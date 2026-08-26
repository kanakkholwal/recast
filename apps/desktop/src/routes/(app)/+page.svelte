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

<StudioPage title="Record">
  <div class="relative mx-auto flex min-h-full w-full max-w-2xl flex-col justify-center gap-8 py-10">
    <div
      aria-hidden="true"
      class="bg-grid-pattern pointer-events-none absolute -inset-x-16 -top-16 -z-10 h-[26rem] [mask-image:radial-gradient(ellipse_65%_65%_at_50%_35%,black,transparent)]"
    ></div>

    <div class="flex flex-col items-center gap-1.5 text-center">
      <h2 class="text-[26px] font-bold tracking-tight text-foreground">{hello}</h2>
      <p class="text-[13px] text-muted-foreground">What would you like to capture?</p>
    </div>

    <!-- One recorder deck: the four capture modes as hairline-divided tiles
         inside a single elevated surface, so the page reads as an instrument
         rather than a grid of web cards. gap-px over a border-tinted backdrop
         draws the dividers on both axes at every breakpoint. -->
    <div
      class="overflow-hidden rounded-3xl border border-border/50 bg-border/40 shadow-craft-md"
    >
      <div class="grid grid-cols-2 gap-px sm:grid-cols-4">
        {#each modes as mode (mode.intent)}
          {@const Icon = mode.icon}
          <button
            type="button"
            onclick={() => launchRecordingPanel(mode.intent)}
            class="group/mode relative flex flex-col items-center gap-3 bg-card px-3 py-7 text-center transition-colors duration-200 ease-out hover:bg-muted/40 focus:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/50 motion-safe:active:scale-[0.98]"
          >
            <span
              class="flex size-11 items-center justify-center rounded-xl bg-muted/60 text-foreground ring-1 ring-inset ring-border/40 transition-transform duration-200 ease-out motion-safe:group-hover/mode:scale-110 motion-safe:group-hover/mode:-translate-y-0.5"
            >
              <Icon class="size-5" />
            </span>
            <span class="flex flex-col gap-0.5">
              <span class="text-[12.5px] font-semibold tracking-tight text-foreground">
                {mode.label}
              </span>
              <span class="text-[10.5px] leading-snug text-muted-foreground">{mode.hint}</span>
            </span>
            <ArrowUpRight
              class="absolute right-2.5 top-2.5 size-3.5 text-muted-foreground/40 opacity-0 transition-all duration-200 ease-out group-hover/mode:text-foreground group-hover/mode:opacity-100 motion-safe:group-hover/mode:-translate-y-0.5 motion-safe:group-hover/mode:translate-x-0.5"
            />
          </button>
        {/each}
      </div>
    </div>

    <p class="text-center text-[11.5px] text-muted-foreground/70">
      Pick a mode to open the recorder, or press
      <kbd class="rounded border border-border/60 bg-card px-1 py-0.5 font-mono text-[10px] text-foreground/80">{recordShortcut}</kbd>
    </p>
  </div>
</StudioPage>
