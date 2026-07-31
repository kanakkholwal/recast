<script lang="ts">
import { MousePointer2 } from "@recast/icons";

interface Props {
	/** Current export stage; drives which themed glyph + ring mode shows. */
	stage: "prepare" | "render" | "finalise";
	/** 0..100; fills the determinate ring during `render`. */
	pct?: number;
}
let { stage, pct = 0 }: Props = $props();

const RING_R = 52;
const clamped = $derived(Math.min(100, Math.max(0, pct)));
</script>

<div
	class="relative size-32"
	role="progressbar"
	aria-label="Export progress"
	aria-valuemin={0}
	aria-valuemax={100}
	aria-valuenow={stage === "render" ? Math.floor(clamped) : undefined}
	aria-valuetext={stage === "render"
		? `${Math.floor(clamped)}%`
		: stage === "prepare"
			? "Preparing"
			: "Finalising"}
>
	<svg viewBox="0 0 120 120" class="size-full -rotate-90 overflow-visible">
		<circle
			cx="60"
			cy="60"
			r={RING_R}
			stroke="currentColor"
			stroke-width="6"
			class="fill-none text-muted"
		/>
		{#if stage === "render"}
			<circle
				cx="60"
				cy="60"
				r={RING_R}
				pathLength="100"
				stroke="currentColor"
				stroke-width="6"
				stroke-linecap="round"
				class="fill-none text-primary"
				style="stroke-dasharray:100; stroke-dashoffset:{100 -
					clamped}; transition:stroke-dashoffset 220ms cubic-bezier(0.65,0,0.35,1);"
			/>
		{:else}
			<!-- Indeterminate arc for the un-measurable stages (prep / mux). -->
			<circle
				cx="60"
				cy="60"
				r={RING_R}
				pathLength="100"
				stroke="currentColor"
				stroke-width="6"
				stroke-linecap="round"
				class="origin-center fill-none text-primary motion-safe:animate-spin"
				style="stroke-dasharray:25 100; animation-duration:1.2s;"
			/>
		{/if}
	</svg>

	<div class="absolute inset-0 grid place-items-center">
		{#if stage === "render"}
			<!-- The ring + counting number is the animation; keep the centre clean.
			     `%` is absolute so digit count never shifts the number off-centre. -->
			<div class="relative font-mono leading-none tabular-nums">
				<span class="text-[2rem] font-semibold text-foreground">{Math.floor(clamped)}</span>
				<span class="absolute -right-3.5 top-0.5 text-sm font-medium text-muted-foreground">%</span>
			</div>
		{:else if stage === "prepare"}
			<!-- Snapshotting the scene: a cursor glides + clicks over the frame. -->
			<div class="relative grid size-11 place-items-center">
				<span class="size-8 rounded-md border-2 border-primary/30"></span>
				<span class="cursor-glide absolute text-primary">
					<MousePointer2 class="size-4" />
				</span>
			</div>
		{:else}
			<!-- Muxing audio in: an equalizer settles into the track. -->
			<span class="eq flex h-8 items-center gap-1 text-primary">
				<i></i><i></i><i></i><i></i>
			</span>
		{/if}
	</div>
</div>

<style>
	@keyframes cursor-glide {
		0% {
			transform: translate(-7px, -6px) scale(1);
			opacity: 0.4;
		}
		30% {
			transform: translate(6px, 5px) scale(1);
			opacity: 1;
		}
		40% {
			transform: translate(6px, 5px) scale(0.82);
			opacity: 1;
		}
		50% {
			transform: translate(6px, 5px) scale(1);
			opacity: 1;
		}
		80% {
			transform: translate(-6px, -5px) scale(1);
			opacity: 0.7;
		}
		100% {
			transform: translate(-7px, -6px) scale(1);
			opacity: 0.4;
		}
	}
	.cursor-glide {
		animation: cursor-glide 2.6s ease-in-out infinite;
	}

	.eq i {
		width: 3px;
		height: 100%;
		border-radius: 2px;
		background: currentColor;
		transform-origin: center;
	}
	.eq i:nth-child(1) {
		animation: eq 0.9s ease-in-out infinite;
	}
	.eq i:nth-child(2) {
		animation: eq 0.9s ease-in-out infinite 0.15s;
	}
	.eq i:nth-child(3) {
		animation: eq 0.9s ease-in-out infinite 0.3s;
	}
	.eq i:nth-child(4) {
		animation: eq 0.9s ease-in-out infinite 0.45s;
	}
	@keyframes eq {
		0%,
		100% {
			transform: scaleY(0.35);
		}
		50% {
			transform: scaleY(1);
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.cursor-glide,
		.eq i {
			animation: none;
		}
		.cursor-glide {
			transform: translate(0, 0);
			opacity: 0.85;
		}
		.eq i {
			transform: scaleY(0.6);
		}
	}
</style>
