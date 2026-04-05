<script lang="ts">
	import type { EditorStore } from "$lib/stores/editor-store.svelte";
	import { fade } from "svelte/transition";

	interface Props {
		store: EditorStore;
		previewSrc: string;
		fallbackSrc: string;
		isRendering: boolean;
	}

	let { store, previewSrc, fallbackSrc, isRendering }: Props = $props();

	const activeSrc = $derived(previewSrc || fallbackSrc);
</script>

<div class="relative flex h-full w-full max-w-[1120px] items-center justify-center overflow-hidden rounded-[24px] border border-border/60 bg-[radial-gradient(circle_at_top,rgba(59,130,246,0.12),transparent_42%),linear-gradient(180deg,rgba(10,14,23,0.96),rgba(5,7,12,0.98))] shadow-[0_18px_60px_rgba(0,0,0,0.35)]">
	{#if activeSrc}
		<img
			in:fade={{ duration: 180 }}
			src={activeSrc}
			alt="Rendered preview"
			class="max-h-full max-w-full object-contain"
			draggable="false"
		/>
	{:else}
		<div class="flex h-full w-full items-center justify-center">
			<div class="rounded-2xl border border-white/10 bg-white/5 px-4 py-3 text-sm text-white/70 backdrop-blur-sm">
				Preview is being prepared
			</div>
		</div>
	{/if}

	{#if isRendering}
		<div class="pointer-events-none absolute right-4 top-4 rounded-full border border-white/10 bg-black/45 px-3 py-1.5 text-[11px] font-medium text-white/85 backdrop-blur-sm">
			Rendering...
		</div>
	{/if}
</div>
