<script lang="ts">
import { Check, FileVideo, FolderOpen, Link2 } from "@recast/icons";
import { buttonVariants } from "@recast/ui/button";
import { cn } from "@recast/ui/utils";
import { prefersReducedMotion } from "$lib/motion-core";

// Export, upload, link: three hairline stages under one file, ending in the
// only thing the visitor actually wants, a link they can paste.
const reduced = $derived(prefersReducedMotion());

const STEP_MS = 1600;
const HOLD_MS = 2800;
const LOOP_MS = STEP_MS * 3 + HOLD_MS;

let elapsed = $state(0);

$effect(() => {
	if (reduced) return;
	const id = setInterval(() => {
		if (!document.hidden) elapsed = (elapsed + 50) % LOOP_MS;
	}, 50);
	return () => clearInterval(id);
});

const done = $derived(reduced ? 3 : Math.min(3, Math.floor(elapsed / STEP_MS)));
const partial = $derived(reduced ? 0 : Math.min(1, (elapsed % STEP_MS) / STEP_MS));
const percent = $derived(
	reduced ? 100 : Math.min(100, Math.round(((done + (done < 3 ? partial : 0)) / 3) * 100)),
);

const stages = [
	{ label: "Export", pending: "Queued", running: "Encoding", complete: "1080p" },
	{ label: "Upload", pending: "Waiting", running: "Sending", complete: "12.4 MB" },
	{ label: "Link", pending: "Waiting", running: "Signing", complete: "Ready" },
] as const;

const linkReady = $derived(done >= 3);
</script>

<div class="p-4">
	<div class="flex items-center gap-2.5 pb-3">
		<FileVideo class="size-4 shrink-0 text-tag-green [fill-opacity:0.2]" fill="currentColor" />
		<span class="min-w-0 flex-1 truncate text-body-sm font-medium text-foreground">
			launch-demo.mp4
		</span>
		<span class="shrink-0 text-caption tabular-nums text-muted-foreground">
			{percent}%
		</span>
	</div>

	<!-- One hairline track carries the whole transfer; no stage gets a bar of its own. -->
	<div aria-hidden="true" class="h-px w-full bg-border-low">
		<div
			class="h-px bg-tag-green transition-[width] duration-100 ease-linear motion-reduce:transition-none"
			style={`width:${percent}%`}
		></div>
	</div>

	<ol class="mt-px grid grid-cols-3 gap-px border-b border-border-low bg-border-low">
		{#each stages as stage, i (stage.label)}
			{@const complete = done > i}
			{@const running = done === i}
			<li class="bg-card px-3 py-2.5">
				<div class="flex items-center gap-1.5">
					<span
						class={cn(
							"size-1.5 shrink-0 rounded-full transition-colors duration-300 motion-reduce:transition-none",
							complete ? "bg-tag-green" : running ? "bg-foreground" : "bg-border-strong",
						)}
					></span>
					<span
						class={cn(
							"truncate text-caption font-medium transition-colors duration-300 motion-reduce:transition-none",
							complete || running ? "text-foreground" : "text-muted-foreground",
						)}
					>
						{stage.label}
					</span>
					{#if complete}
						<Check class="ml-auto size-3 shrink-0 text-tag-green" />
					{/if}
				</div>
				<div class="mt-1 truncate text-caption text-muted-foreground">
					{complete ? stage.complete : running ? stage.running : stage.pending}
				</div>
			</li>
		{/each}
	</ol>

	<!-- Destination and link. Always rendered so nothing shifts when it resolves. -->
	<div class="flex items-center gap-2.5 pt-3">
		<FolderOpen class="size-4 shrink-0 text-muted-foreground" />
		<span class="shrink-0 text-caption text-muted-foreground">My Drive / Recast</span>
		<span
			class={cn(
				"min-w-0 flex-1 truncate text-right text-caption tracking-tight transition-colors duration-300 motion-reduce:transition-none",
				linkReady ? "text-foreground" : "text-border-strong",
			)}
		>
			{linkReady ? "recast.li/d/8fk2a" : "Generating link"}
		</span>
		<!-- Inert: a mock control must not take focus. -->
		<span
			aria-hidden="true"
			class={cn(
				buttonVariants({ variant: "outline", size: "xs" }),
				"pointer-events-none shrink-0 transition-opacity duration-300 motion-reduce:transition-none",
				linkReady ? "opacity-100" : "opacity-40",
			)}
		>
			<Link2 class="size-3" />
			Copy link
		</span>
	</div>
</div>
