<script lang="ts">
	import {
		formatBytes,
		formatDuration,
		formatRelative,
	} from "$lib/dashboard/format";
	import type { Recording } from "$lib/dashboard/store.svelte";
	import { Clock, Cloud, MonitorPlay, Video, X } from "lucide-svelte";
	import { cubicOut } from "svelte/easing";
	import { fade, scale } from "svelte/transition";

	let {
		recording,
		onclose,
	}: {
		recording: Recording;
		onclose: () => void;
	} = $props();
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />

<div class="fixed inset-0 z-[100] grid place-items-center p-4 sm:p-8">
	<button
		type="button"
		aria-label="Close player"
		onclick={onclose}
		class="absolute inset-0 cursor-default bg-background/80 backdrop-blur-sm"
		transition:fade={{ duration: 150 }}
	></button>

	<div
		class="glass-card relative z-10 w-full max-w-3xl overflow-hidden rounded-2xl shadow-craft-xl"
		transition:scale={{ start: 0.96, duration: 240, easing: cubicOut }}
	>
		<header class="flex items-center gap-3 border-b border-border-low/50 px-4 py-3">
			<Video class="size-4 shrink-0 text-primary" />
			<span class="min-w-0 flex-1 truncate text-sm font-semibold text-foreground">
				{recording.title}
			</span>
			<button
				type="button"
				onclick={onclose}
				aria-label="Close"
				class="grid size-7 place-items-center rounded-md text-muted-foreground transition-colors hover:bg-foreground/8 hover:text-foreground"
			>
				<X class="size-4" />
			</button>
		</header>

		<!-- svelte-ignore a11y_media_has_caption -->
		<video
			src={recording.videoUrl}
			poster={recording.posterUrl || undefined}
			controls
			autoplay
			class="aspect-video w-full bg-black"
		></video>

		<footer class="flex flex-wrap items-center gap-x-4 gap-y-1 px-4 py-3 text-xs text-muted-foreground">
			<span class="flex items-center gap-1.5">
				<Clock class="size-3.5" />
				{formatDuration(recording.durationSec)}
			</span>
			<span>{formatBytes(recording.sizeBytes)}</span>
			<span>{formatRelative(recording.createdAt)}</span>
			<span class="flex items-center gap-1.5">
				{#if recording.source === "cloud"}
					<Cloud class="size-3.5 text-primary" />{recording.provider}
				{:else}
					<MonitorPlay class="size-3.5" />Local
				{/if}
			</span>
		</footer>
	</div>
</div>
