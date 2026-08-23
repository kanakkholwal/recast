<script lang="ts">
import { Check, ChevronsLeftRight, Film, Pause, Play } from "@recast/icons";
import { autoplayInView } from "$lib/motion-core";

// Draggable wipe comparison. Raw fills the base; the polished clip is layered
// on top and clipped to the RIGHT of the handle, so the left half is always the
// raw take and the right half is always the polished one. Both clips autoplay
// and loop independently: they are deliberately NOT frame-synced, because
// polishing changes the video (silence-trim shortens it), which is the point.
type Clip = {
	src: string;
	poster?: string;
	label: string;
	durationLabel: string;
};
let { raw, polished, applied = [] }: { raw: Clip; polished: Clip; applied?: string[] } = $props();

const ready = $derived(raw.src.length > 0 && polished.src.length > 0);

// Handle position, percent from the left edge. Everything left of it is raw.
let pos = $state(52);
let root: HTMLElement | undefined = $state();
let dragging = $state(false);

// Each side's chrome is clipped to the same geometry as its own video, so a
// label never sits over the footage it isn't describing.
const rawClip = $derived(`inset(0 ${100 - pos}% 0 0)`);
const polishedClip = $derived(`inset(0 0 0 ${pos}%)`);

// WCAG 2.2.2: auto-playing motion longer than five seconds needs a control.
let rawVideo = $state<HTMLVideoElement | null>(null);
let polishedVideo = $state<HTMLVideoElement | null>(null);
let userPaused = $state(false);

function togglePlayback() {
	userPaused = !userPaused;
	for (const v of [rawVideo, polishedVideo]) {
		if (!v) continue;
		if (userPaused) v.pause();
		else void v.play().catch(() => {});
	}
}

/** `autoplayInView` resumes on scroll, which would undo a manual pause. */
function holdPause(e: Event) {
	if (userPaused) (e.currentTarget as HTMLVideoElement).pause();
}

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
	<figure class="flex flex-col gap-4">
		<!-- The play control is a sibling of the slider, not a child: interactive
		     content inside a `slider` role is not exposed reliably. -->
		<div class="relative">
			<div
			bind:this={root}
			role="slider"
			tabindex="0"
			aria-label="Compare the raw recording with the polished result"
			aria-orientation="horizontal"
			aria-valuemin={0}
			aria-valuemax={100}
			aria-valuenow={Math.round(pos)}
			aria-valuetext={`${Math.round(pos)}% raw, ${Math.round(100 - pos)}% polished`}
			onpointerdown={onPointerDown}
			onpointermove={onPointerMove}
			onpointerup={stop}
			onpointercancel={stop}
			onkeydown={onKeydown}
			class="group relative aspect-video w-full cursor-ew-resize touch-pan-y overflow-hidden rounded-xl border border-border-low bg-background outline-none select-none focus-visible:ring-2 focus-visible:ring-primary"
		>
			<!-- svelte-ignore a11y_media_has_caption -->
			<video
				bind:this={rawVideo}
				use:autoplayInView
				onplay={holdPause}
				src={raw.src}
				poster={raw.poster}
				autoplay
				loop
				muted
				playsinline
				preload="metadata"
				class="pointer-events-none absolute inset-0 size-full object-cover"
				draggable="false"
			></video>
			<!-- svelte-ignore a11y_media_has_caption -->
			<video
				bind:this={polishedVideo}
				use:autoplayInView
				onplay={holdPause}
				src={polished.src}
				poster={polished.poster}
				autoplay
				loop
				muted
				playsinline
				preload="metadata"
				class="pointer-events-none absolute inset-0 size-full object-cover"
				style={`clip-path: ${polishedClip};`}
				draggable="false"
			></video>

			<!-- Raw-side chrome, clipped to the raw side. -->
			<div
				aria-hidden="true"
				class="pointer-events-none absolute inset-0"
				style={`clip-path: ${rawClip};`}
			>
				<span
					class="absolute left-3 top-3 rounded-full bg-black/60 px-2.5 py-1 text-caption font-medium text-white ring-1 ring-inset ring-white/20"
				>
					{raw.label}
				</span>
				<span
					class="absolute bottom-3 left-3 rounded-md bg-black/60 px-1.5 py-0.5 text-caption font-medium tabular-nums text-white ring-1 ring-inset ring-white/20"
				>
					{raw.durationLabel}
				</span>
			</div>

			<!-- Polished-side chrome, clipped to the polished side. -->
			<div
				aria-hidden="true"
				class="pointer-events-none absolute inset-0"
				style={`clip-path: ${polishedClip};`}
			>
				<span
					class="absolute right-3 top-3 rounded-full bg-white px-2.5 py-1 text-caption font-medium text-black"
				>
					{polished.label}
				</span>
				<span
					class="absolute bottom-3 right-3 rounded-md bg-white px-1.5 py-0.5 text-caption font-medium tabular-nums text-black"
				>
					{polished.durationLabel}
				</span>
			</div>

			<!-- Handle. Decorative: the root element is the actual slider. -->
			<div
				aria-hidden="true"
				class="pointer-events-none absolute inset-y-0 z-10"
				style={`left: ${pos}%;`}
			>
				<div class="absolute inset-y-0 left-1/2 w-0.5 -translate-x-1/2 bg-white"></div>
				<div
					class="absolute left-1/2 top-1/2 grid size-10 -translate-x-1/2 -translate-y-1/2 place-items-center rounded-full bg-white text-black transition-transform group-active:scale-95 motion-reduce:transition-none"
				>
					<ChevronsLeftRight class="size-4" />
				</div>
			</div>
			</div>

			<!-- Sits over the frame but outside the drag surface, so it can be
			     clicked and tabbed to without moving the handle. -->
			<button
				type="button"
				onclick={togglePlayback}
				onpointerdown={(e) => e.stopPropagation()}
				aria-label={userPaused ? "Play both clips" : "Pause both clips"}
				class="absolute bottom-3 left-1/2 z-20 grid size-9 -translate-x-1/2 cursor-pointer place-items-center rounded-full bg-black/60 text-white ring-1 ring-inset ring-white/20 outline-none transition-colors hover:bg-black/75 focus-visible:ring-2 focus-visible:ring-white motion-reduce:transition-none"
			>
				{#if userPaused}
					<Play class="size-4 translate-x-px fill-current" />
				{:else}
					<Pause class="size-4 fill-current" />
				{/if}
			</button>
		</div>

		<!-- What Recast changed. These belong to the polished cut, so they sit
		     under the frame rather than floating over footage that may be the raw
		     take at the current handle position. -->
		{#if applied.length > 0}
			<figcaption class="flex flex-wrap items-center gap-x-4 gap-y-2">
				<span class="text-caption text-muted-foreground">Applied automatically</span>
				{#each applied as feat (feat)}
					<span class="inline-flex items-center gap-1.5 text-body-sm text-foreground">
						<Check class="size-3.5 shrink-0 text-tag-green" />
						{feat}
					</span>
				{/each}
			</figcaption>
		{/if}
	</figure>
{:else}
	<!-- Empty state: renders when a clip URL is not wired yet, so the section
	     reads as intentional rather than broken. -->
	<div
		class="relative grid aspect-video w-full place-items-center overflow-hidden rounded-xl border border-border-low bg-background"
	>
		<div
			aria-hidden="true"
			class="pointer-events-none absolute inset-0 opacity-40"
			style="background-image: radial-gradient(circle, color-mix(in srgb, var(--color-foreground) 10%, transparent) 1px, transparent 1px); background-size: 18px 18px;"
		></div>
		<div class="relative flex flex-col items-center gap-3 text-center">
			<Film class="size-6 text-border-strong" />
			<div class="text-caption text-muted-foreground">Clip pending</div>
		</div>
	</div>
{/if}
