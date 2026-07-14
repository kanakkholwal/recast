<script lang="ts">
	import { autoplayInView } from "$lib/motion-core";
	import { Check, ChevronsLeftRight, Film } from "@lucide/svelte";
	import { cn } from "@recast/ui/utils";

	// Draggable wipe comparison. Raw fills the base; the polished clip is layered
	// on top and clipped to the RIGHT of the handle, so dragging left/right wipes
	// between them. Both clips autoplay at the same time and loop independently:
	// they are deliberately NOT frame-synced because polishing changes the video
	// (silence-trim shortens it), which is exactly the point. The comparison is of
	// the persistent look (framing, padding, zoom, smoothed cursor), so it reads
	// at any handle position, and the length delta is surfaced as proof, not hidden.
	type Clip = {
		src: string;
		poster?: string;
		label: string;
		durationLabel: string;
	};
	let {
		raw,
		polished,
		applied = [],
	}: { raw: Clip; polished: Clip; applied?: string[] } = $props();

	const ready = $derived(raw.src.length > 0 && polished.src.length > 0);

	// Handle position, percent from the left edge.
	let pos = $state(52);
	let root: HTMLElement | undefined = $state();
	let dragging = $state(false);

	function setFromX(clientX: number) {
		if (!root) return;
		const rect = root.getBoundingClientRect();
		pos = Math.min(100, Math.max(0, ((clientX - rect.left) / rect.width) * 100));
	}
	function onPointerDown(e: PointerEvent) {
		dragging = true;
		root?.setPointerCapture(e.pointerId);
		setFromX(e.clientX);
	}
	function onPointerMove(e: PointerEvent) {
		if (dragging) setFromX(e.clientX);
	}
	function stop() {
		dragging = false;
	}
	function onKeydown(e: KeyboardEvent) {
		const step = e.shiftKey ? 10 : 2;
		if (e.key === "ArrowLeft") pos = Math.max(0, pos - step);
		else if (e.key === "ArrowRight") pos = Math.min(100, pos + step);
		else if (e.key === "Home") pos = 0;
		else if (e.key === "End") pos = 100;
		else return;
		e.preventDefault();
	}
</script>

{#if ready}
	<div
		bind:this={root}
		role="slider"
		tabindex="0"
		aria-label="Drag to compare the raw recording with the polished result"
		aria-orientation="horizontal"
		aria-valuemin={0}
		aria-valuemax={100}
		aria-valuenow={Math.round(pos)}
		aria-valuetext={`${Math.round(pos)}% polished revealed`}
		onpointerdown={onPointerDown}
		onpointermove={onPointerMove}
		onpointerup={stop}
		onpointercancel={stop}
		onkeydown={onKeydown}
		class="group relative aspect-video w-full cursor-ew-resize touch-pan-y overflow-hidden rounded-2xl border border-primary/40 bg-background shadow-craft-xl ring-1 ring-primary/20 outline-none select-none focus-visible:ring-2 focus-visible:ring-primary"
	>
		<!-- svelte-ignore a11y_media_has_caption -->
		<video
			use:autoplayInView
			src={raw.src}
			poster={raw.poster}
			autoplay
			loop
			muted
			playsinline
			preload="metadata"
			class="pointer-events-none absolute inset-0 size-full object-cover saturate-[0.85]"
		></video>
		<!-- svelte-ignore a11y_media_has_caption -->
		<video
			use:autoplayInView
			src={polished.src}
			poster={polished.poster}
			autoplay
			loop
			muted
			playsinline
			preload="metadata"
			class="pointer-events-none absolute inset-0 size-full object-cover"
			style={`clip-path: inset(0 0 0 ${pos}%);`}
		></video>

		<!-- Corner labels -->
		<span
			class="pointer-events-none absolute left-3 top-3 rounded-full bg-black/55 px-2.5 py-1 text-[10px] font-bold uppercase tracking-[0.16em] text-white/85 ring-1 ring-inset ring-white/15 backdrop-blur"
		>
			{raw.label}
		</span>
		<span
			class="pointer-events-none absolute right-3 top-3 rounded-full bg-primary/20 px-2.5 py-1 text-[10px] font-bold uppercase tracking-[0.16em] text-primary ring-1 ring-inset ring-primary/30 backdrop-blur"
		>
			{polished.label}
		</span>

		<!-- Applied-feature chips, attributed to the polished side. -->
		{#if applied.length > 0}
			<div
				class="pointer-events-none absolute inset-x-0 bottom-10 flex flex-wrap items-center justify-center gap-1.5 px-4"
			>
				{#each applied as feat (feat)}
					<span
						class="inline-flex items-center gap-1 rounded-full bg-black/55 px-2 py-0.5 text-[10px] font-semibold text-white/90 ring-1 ring-inset ring-white/15 backdrop-blur"
					>
						<Check class="size-2.5 text-primary" />
						{feat}
					</span>
				{/each}
			</div>
		{/if}

		<!-- Duration delta: the polished cut lands shorter once silence is trimmed.
		     Surfaced as proof rather than hidden. -->
		<span
			class="pointer-events-none absolute bottom-3 left-3 rounded-md bg-black/55 px-1.5 py-0.5 font-mono text-[10px] font-semibold text-white/80 ring-1 ring-inset ring-white/15 backdrop-blur"
		>
			{raw.durationLabel}
		</span>
		<span
			class="pointer-events-none absolute bottom-3 right-3 rounded-md bg-primary/15 px-1.5 py-0.5 font-mono text-[10px] font-semibold text-primary ring-1 ring-inset ring-primary/30 backdrop-blur"
		>
			{polished.durationLabel}
		</span>

		<!-- Handle. Decorative: the root element is the actual slider. -->
		<div
			aria-hidden="true"
			class="pointer-events-none absolute inset-y-0 z-10"
			style={`left: ${pos}%;`}
		>
			<div
				class="absolute inset-y-0 left-1/2 w-0.5 -translate-x-1/2 bg-white/80"
			></div>
			<div
				class="absolute left-1/2 top-1/2 grid size-10 -translate-x-1/2 -translate-y-1/2 place-items-center rounded-full bg-white text-black shadow-craft-lg ring-1 ring-black/10 transition-transform group-active:scale-95"
			>
				<ChevronsLeftRight class="size-4" />
			</div>
		</div>
	</div>
{:else}
	<!-- Empty state: renders when a clip URL is not wired yet, so the section
	     reads as intentional rather than broken. Mirrors the editor-rail card. -->
	<div
		class="relative grid aspect-video w-full place-items-center overflow-hidden rounded-2xl border border-border/60 bg-background"
	>
		<div
			aria-hidden="true"
			class="pointer-events-none absolute inset-0 opacity-40"
			style="background-image: radial-gradient(circle, color-mix(in srgb, var(--color-foreground) 10%, transparent) 1px, transparent 1px); background-size: 18px 18px;"
		></div>
		<div
			aria-hidden="true"
			class="pointer-events-none absolute left-1/2 top-1/2 size-72 -translate-x-1/2 -translate-y-1/2 rounded-full opacity-60"
			style="background: radial-gradient(closest-side, color-mix(in srgb, var(--color-primary) 18%, transparent), transparent 70%);"
		></div>
		<div class="relative flex flex-col items-center gap-3 text-center">
			<span
				class="grid size-14 place-items-center rounded-2xl border border-primary/30 bg-primary/10 text-primary backdrop-blur-sm"
			>
				<Film class="size-6" />
			</span>
			<div
				class="font-mono text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground"
			>
				Clip pending
			</div>
		</div>
	</div>
{/if}
