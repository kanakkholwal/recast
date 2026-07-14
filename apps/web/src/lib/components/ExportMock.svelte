<script lang="ts">
	import { prefersReducedMotion } from "$lib/motion-core";
	import { Check, HardDriveUpload, Link2, LoaderCircle } from "@lucide/svelte";

	// Step 3's export-success mock. Loops the real flow: the encode finishes, the
	// upload progress fills, then it lands as "Uploaded to Drive" with Copy link,
	// holds, and repeats. Reduced motion shows the finished state (the meaningful
	// end), no spinner or progress animation. Same restrained cadence as Steps 1-2.
	const reduced = $derived(prefersReducedMotion());

	const UPLOAD_TICKS = 30; // ~1.8s filling
	const TOTAL_TICKS = 72; // + ~2.5s holding on "done"
	let tick = $state(0);

	$effect(() => {
		if (reduced) return;
		const id = setInterval(() => {
			if (!document.hidden) tick = (tick + 1) % TOTAL_TICKS;
		}, 60);
		return () => clearInterval(id);
	});

	const done = $derived(reduced || tick >= UPLOAD_TICKS);
	const progress = $derived(
		reduced ? 100 : Math.min(100, Math.round((tick / UPLOAD_TICKS) * 100)),
	);
</script>

<div
	class="rounded-xl border border-border-low/70 bg-background/80 p-4 shadow-craft-inset"
>
	<div class="flex items-start gap-3">
		<span
			class="grid size-9 shrink-0 place-items-center rounded-lg border border-success/30 bg-success/10 text-success"
		>
			<Check class="size-4" />
		</span>
		<div class="min-w-0 flex-1">
			<div class="text-[13px] font-semibold tracking-tight text-foreground">
				Export complete
			</div>
			<div class="mt-0.5 truncate font-mono text-[10px] text-muted-foreground">
				~/Recordings/launch-demo.mp4
			</div>
		</div>
	</div>

	<div class="mt-3 rounded-lg border border-border-low/60 bg-foreground/2 px-3 py-2">
		{#if done}
			<div class="flex items-center gap-2">
				<HardDriveUpload class="size-3.5 shrink-0 text-success" />
				<span class="text-[11.5px] font-medium text-foreground">Uploaded to Drive</span>
				<span
					class="ml-auto inline-flex items-center gap-1 rounded-md border border-border-low/60 bg-background px-1.5 py-0.5 text-[10px] font-semibold text-foreground"
				>
					<Link2 class="size-3 text-primary" />
					Copy link
				</span>
			</div>
		{:else}
			<div class="flex items-center gap-2">
				<LoaderCircle class="size-3.5 shrink-0 animate-spin text-primary" />
				<span class="text-[11.5px] font-medium text-foreground">Uploading to Drive…</span>
				<span class="ml-auto font-mono text-[10px] font-semibold text-muted-foreground">
					{progress}%
				</span>
			</div>
			<div class="mt-2 h-1 overflow-hidden rounded-full bg-border-low/60">
				<div
					class="h-full rounded-full bg-primary transition-[width] duration-100"
					style={`width: ${progress}%;`}
				></div>
			</div>
		{/if}
	</div>
</div>
