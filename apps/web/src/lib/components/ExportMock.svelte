<script lang="ts">
import { prefersReducedMotion } from "$lib/motion-core";
import { Check, HardDriveUpload, Link2, Video } from "@recast/icons";
import { cn } from "@recast/ui/utils";

// Export → upload → link, as the three-stop journey it actually is.
//
// The old mock was a success toast with a progress bar bolted under it, so the
// payoff (a link you can paste) was the smallest thing on screen. Here the rail
// fills top to bottom and the link row is the destination: the last stop is the
// only one that gets a surface and an action.
//
// Reduced motion renders the finished state — all three stops done, link
// present — because that is the meaningful end, not a frozen spinner.
const reduced = $derived(prefersReducedMotion());

const STEP_MS = 1500;
const HOLD_MS = 2600;
const LOOP_MS = STEP_MS * 3 + HOLD_MS;

let elapsed = $state(0);

$effect(() => {
	if (reduced) return;
	const id = setInterval(() => {
		if (!document.hidden) elapsed = (elapsed + 50) % LOOP_MS;
	}, 50);
	return () => clearInterval(id);
});

const stops = [
	{ icon: Video, label: "Export complete", meta: "launch-demo.mp4 · 12.4 MB" },
	{ icon: HardDriveUpload, label: "Uploaded to Drive", meta: "My Drive / Recast" },
	{ icon: Link2, label: "Link ready", meta: "recast.li/d/8fk2a" },
] as const;

// How many stops are done, and how far the rail has filled into the next one.
const done = $derived(reduced ? 3 : Math.min(3, Math.floor(elapsed / STEP_MS)));
const partial = $derived(reduced ? 0 : Math.min(1, (elapsed % STEP_MS) / STEP_MS));

// Rail fill as a percentage of its full height. Each stop owns a third.
const fill = $derived(reduced ? 100 : Math.min(100, ((done + (done < 3 ? partial : 0)) / 3) * 100));
</script>

<div class="rounded-xl border border-border-low bg-card p-4">
	<div class="relative">
		<!-- Rail. One track, one fill — the fill is the progress indicator, so no
		     stop needs a bar of its own. -->
		<div
			aria-hidden="true"
			class="absolute left-3.25 top-3 bottom-3 w-px bg-border-low"
		></div>
		<div
			aria-hidden="true"
			class="absolute left-3.25 top-3 w-px bg-tag-green transition-[height] duration-100 ease-linear motion-reduce:transition-none"
			style={`height: calc((100% - 1.5rem) * ${fill} / 100)`}
		></div>

		<ol class="relative space-y-3.5">
			{#each stops as stop, i (stop.label)}
				{@const complete = done > i}
				{@const active = done === i}
				{@const Icon = stop.icon}
				<li class="flex items-start gap-3">
					<span
						class={cn(
							"relative z-10 grid size-7 shrink-0 place-items-center rounded-full border bg-card transition-colors duration-300 motion-reduce:transition-none",
							complete
								? "border-tag-green text-tag-green"
								: active
									? "border-border-strong text-foreground"
									: "border-border-low text-border-strong",
						)}
					>
						{#if complete}
							<Check class="size-3.5" />
						{:else}
							<Icon class="size-3.5" />
						{/if}
					</span>

					<div class="min-w-0 flex-1 pt-0.5">
						<div
							class={cn(
								"text-caption font-semibold transition-colors duration-300 motion-reduce:transition-none",
								complete || active ? "text-foreground" : "text-border-strong",
							)}
						>
							{stop.label}
						</div>
						<div
							class={cn(
								"mt-0.5 truncate font-mono text-caption transition-colors duration-300 motion-reduce:transition-none",
								complete || active ? "text-muted-foreground" : "text-border-strong",
							)}
						>
							{stop.meta}
						</div>
					</div>

					<!-- Only the destination carries an action. -->
					{#if i === 2}
						<span
							class={cn(
								"mt-0.5 inline-flex shrink-0 items-center gap-1.5 rounded-md border px-2 py-1 text-caption font-medium transition-all duration-300 motion-reduce:transition-none",
								complete
									? "border-border-low bg-background text-foreground opacity-100"
									: "border-transparent text-transparent opacity-0",
							)}
						>
							<Link2 class="size-3 text-muted-foreground" />
							Copy link
						</span>
					{/if}
				</li>
			{/each}
		</ol>
	</div>
</div>
