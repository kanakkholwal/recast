<script lang="ts">
import StudioPage from "$components/layout/StudioPage.svelte";
import { launchRecordingPanel } from "$lib/ipc";
import { AppWindow, ArrowUpRight, Camera, Crop, Monitor } from "@recast/icons";
import { greeting } from "./home.logic";

const hello = greeting(new Date());

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
  <div class="mx-auto flex min-h-full w-full max-w-xl flex-col justify-center gap-3 py-8">
    {#each modes as mode (mode.intent)}
      {@const Icon = mode.icon}
      <button
        type="button"
        onclick={() => launchRecordingPanel(mode.intent)}
        class="group/mode relative flex items-center gap-4 rounded-2xl border border-border/60 bg-card p-4 pr-5 text-left shadow-(--shadow-craft-inset) transition-[transform,border-color,box-shadow] duration-200 ease-out hover:border-border hover:shadow-craft-md focus:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/50 motion-safe:hover:-translate-y-0.5 motion-safe:active:scale-[0.99]"
      >
        <span
          class="flex size-12 shrink-0 items-center justify-center rounded-xl bg-foreground/5 text-foreground transition-transform duration-200 ease-out motion-safe:group-hover/mode:scale-110"
        >
          <Icon class="size-6" />
        </span>
        <span class="flex min-w-0 flex-1 flex-col gap-0.5">
          <span class="text-[14px] font-semibold text-foreground">{mode.label}</span>
          <span class="text-[12px] text-muted-foreground">{mode.hint}</span>
        </span>
        <ArrowUpRight
          class="size-4 shrink-0 text-muted-foreground/40 transition-all duration-200 ease-out group-hover/mode:text-foreground motion-safe:group-hover/mode:-translate-y-0.5 motion-safe:group-hover/mode:translate-x-0.5"
        />
      </button>
    {/each}
  </div>
</StudioPage>
