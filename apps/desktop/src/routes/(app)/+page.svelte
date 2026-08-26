<script lang="ts">
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

<!-- No StudioPage frame: the nav tab already labels this surface, so Home is a
     full-bleed stage rather than a titled document. -->
<div class="relative h-full min-h-0 overflow-y-auto no-scrollbar">
  <div
    class="relative mx-auto flex min-h-full w-full max-w-2xl flex-col items-center justify-center gap-9 px-6 py-12"
  >
    <!-- Ambient stage light; reads in both themes, unlike the grid pattern. -->
    <div
      aria-hidden="true"
      class="pointer-events-none absolute left-1/2 top-1/2 -z-10 h-[26rem] w-[44rem] max-w-full -translate-x-1/2 -translate-y-1/2 rounded-full bg-foreground/[0.04] blur-3xl"
    ></div>

    <div class="flex flex-col items-center gap-1.5 text-center">
      <h1 class="text-[27px] font-bold tracking-tight text-foreground">{hello}</h1>
      <p class="text-[13px] text-muted-foreground">What would you like to capture?</p>
    </div>

    <!-- Recorder deck: header strip + hairline-divided mode tiles, one instrument. -->
    <div
      class="w-full overflow-hidden rounded-3xl border border-border/60 bg-card shadow-craft-md"
    >
      <div
        class="flex items-center justify-between gap-3 border-b border-border/60 bg-muted/30 px-4 py-2.5"
      >
        <span class="flex items-center gap-2 text-[12px] font-semibold tracking-tight text-foreground">
          <span class="relative flex size-1.5" aria-hidden="true">
            <span class="absolute inline-flex size-full rounded-full bg-destructive/50 motion-safe:animate-ping [animation-duration:2.5s]"></span>
            <span class="relative inline-flex size-1.5 rounded-full bg-destructive"></span>
          </span>
          Start a recording
        </span>
        <span class="flex items-center gap-1.5 text-[11px] text-muted-foreground/80">
          or press
          <kbd
            class="rounded border border-border/60 bg-background/60 px-1.5 py-0.5 font-mono text-[10px] text-foreground/80"
          >
            {recordShortcut}
          </kbd>
        </span>
      </div>

      <div class="grid grid-cols-2 gap-px bg-border/60 sm:grid-cols-4">
        {#each modes as mode (mode.intent)}
          {@const Icon = mode.icon}
          <button
            type="button"
            onclick={() => launchRecordingPanel(mode.intent)}
            class="group/mode relative flex flex-col items-center gap-3 bg-card px-3 py-7 text-center transition-colors duration-200 ease-out hover:bg-muted/40 focus:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/50 motion-safe:active:scale-[0.98]"
          >
            <span
              class="flex size-11 items-center justify-center rounded-xl bg-muted/60 text-foreground ring-1 ring-inset ring-border/50 transition-transform duration-200 ease-out motion-safe:group-hover/mode:scale-110 motion-safe:group-hover/mode:-translate-y-0.5"
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

    <p class="text-center text-[11.5px] leading-relaxed text-muted-foreground/70">
      Recordings land in <span class="font-medium text-muted-foreground">Polish</span>, ready to
      edit. Finished videos live in
      <span class="font-medium text-muted-foreground">Share</span>.
    </p>
  </div>
</div>
