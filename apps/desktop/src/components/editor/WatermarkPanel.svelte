<script lang="ts">
	import type { EditorStore, WatermarkPosition } from "$lib/stores/editor-store.svelte";
	import { ImagePlus, Stamp, Trash2 } from "@lucide/svelte";
	import { toast } from "@recast/ui/sonner";
	import { convertFileSrc } from "@tauri-apps/api/core";
	import InspectorHint from "./InspectorHint.svelte";
	import { SliderControl } from "@recast/ui/slider-control";

	interface Props {
		store: EditorStore;
	}

	const positions: Array<{ value: WatermarkPosition; label: string }> = [
		{ value: "top-left", label: "Top Left" },
		{ value: "top-right", label: "Top Right" },
		{ value: "bottom-left", label: "Bottom Left" },
		{ value: "bottom-right", label: "Bottom Right" },
	];

	let { store }: Props = $props();

	function updateWatermarkSettings(
		updates: Partial<EditorStore["watermarkSettings"]>,
		trackUndo = false,
	) {
		if (trackUndo) {
			store.pushUndoState();
		}
		store.updateWatermarkSettings(updates);
	}

	function getFileLabel(path: string) {
		const segments = path.split(/[/\\]/);
		return segments[segments.length - 1] ?? "Selected image";
	}

	async function handlePickWatermark() {
		const { open } = await import("@tauri-apps/plugin-dialog");
		const selected = await open({
			multiple: false,
			directory: false,
			title: "Choose Watermark Image",
			filters: [
				{
					name: "Images",
					extensions: ["png", "jpg", "jpeg", "webp"],
				},
			],
		});

		if (!selected || typeof selected !== "string") return;

		try {
			updateWatermarkSettings(
				{
					enabled: true,
					imagePath: selected,
					imageSrc: convertFileSrc(selected),
				},
				true,
			);
		} catch (error) {
			toast.error(`Could not load watermark: ${error}`);
		}
	}

	function clearWatermark() {
		updateWatermarkSettings(
			{
				enabled: false,
				imagePath: "",
				imageSrc: "",
			},
			true,
		);
	}
</script>

<div class="flex flex-col gap-4 animate-in fade-in duration-300">
	<section class="rounded-3xl border border-border/70 bg-card/80 p-4 shadow-sm">
		<div class="flex items-start justify-between gap-3">
			<div class="min-w-0">
				<div class="flex items-center gap-2">
					<h3 class="text-sm font-semibold text-foreground">Watermark</h3>
					<InspectorHint content="The watermark is rendered in the preview and exported when an image is selected." />
				</div>
				{#if store.watermarkSettings.imagePath}
					<p class="mt-2 truncate text-xs text-muted-foreground">
						{getFileLabel(store.watermarkSettings.imagePath)}
					</p>
				{:else}
					<p class="mt-2 text-xs text-muted-foreground">No image selected</p>
				{/if}
			</div>

			<button
				type="button"
				onclick={() =>
					updateWatermarkSettings(
						{ enabled: !store.watermarkSettings.enabled },
						true,
					)}
				aria-pressed={store.watermarkSettings.enabled}
				disabled={!store.watermarkSettings.imagePath}
				class="inline-flex items-center gap-2 rounded-2xl border px-3 py-2 text-xs font-semibold transition-colors disabled:cursor-not-allowed disabled:opacity-50 {store.watermarkSettings.enabled
					? 'border-primary/30 bg-primary/10 text-primary'
					: 'border-border/70 bg-background/80 text-muted-foreground hover:text-foreground'}"
			>
				<Stamp size={14} />
				{store.watermarkSettings.enabled ? "Enabled" : "Disabled"}
			</button>
		</div>

		<div class="mt-4 flex gap-2">
			<button
				type="button"
				onclick={handlePickWatermark}
				class="inline-flex flex-1 items-center justify-center gap-2 rounded-2xl border border-border/70 bg-background/80 px-3 py-2 text-xs font-semibold text-foreground transition-colors hover:border-border hover:bg-background"
			>
				<ImagePlus size={14} />
				{store.watermarkSettings.imagePath ? "Replace Image" : "Choose Image"}
			</button>

			{#if store.watermarkSettings.imagePath}
				<button
					type="button"
					onclick={clearWatermark}
					class="inline-flex items-center justify-center rounded-2xl border border-border/70 bg-background/80 px-3 py-2 text-muted-foreground transition-colors hover:border-border hover:text-foreground"
					aria-label="Remove watermark"
				>
					<Trash2 size={14} />
				</button>
			{/if}
		</div>

		{#if store.watermarkSettings.imageSrc}
			<div class="mt-3 overflow-hidden rounded-2xl border border-border/70 bg-background/80">
				<img
					src={store.watermarkSettings.imageSrc}
					alt="Selected watermark"
					class="h-24 w-full object-contain p-3"
				/>
			</div>
		{/if}
	</section>

	{#if store.watermarkSettings.imagePath}
		<section class="rounded-3xl border border-border/70 bg-card/80 p-4 shadow-sm">
			<div class="mb-3 flex items-center gap-2">
				<h4 class="text-sm font-semibold text-foreground">Placement</h4>
				<InspectorHint content="Position and inset are relative to the visible video frame, not the whole editor canvas." />
			</div>

			<div class="grid grid-cols-2 gap-2">
				{#each positions as position}
					<button
						type="button"
						onclick={() =>
							updateWatermarkSettings(
								{ position: position.value },
								store.watermarkSettings.position !== position.value,
							)}
						aria-pressed={store.watermarkSettings.position === position.value}
						class="rounded-2xl border px-3 py-2 text-xs font-semibold transition-all duration-200 {store.watermarkSettings.position ===
						position.value
							? 'border-primary/40 bg-primary/10 text-primary'
							: 'border-border/70 bg-background/70 text-muted-foreground hover:text-foreground'}"
					>
						{position.label}
					</button>
				{/each}
			</div>
		</section>

		<section class="rounded-3xl border border-border/70 bg-card/80 p-4 shadow-sm">
			<div class="space-y-3">
				<SliderControl
					label="Opacity"
					value={store.watermarkSettings.opacity}
					min={10}
					max={100}
					step={5}
					unit="%"
					disabled={!store.watermarkSettings.enabled}
					onstart={() => store.pushUndoState()}
					onchange={(nextValue) => {
						store.updateWatermarkSettings({ opacity: nextValue });
					}}
				/>

				<SliderControl
					label="Size"
					value={store.watermarkSettings.scale}
					min={8}
					max={35}
					step={1}
					unit="%"
					disabled={!store.watermarkSettings.enabled}
					onstart={() => store.pushUndoState()}
					onchange={(nextValue) => {
						store.updateWatermarkSettings({ scale: nextValue });
					}}
				/>

				<SliderControl
					label="Inset"
					value={store.watermarkSettings.inset}
					min={8}
					max={64}
					step={2}
					unit="px"
					disabled={!store.watermarkSettings.enabled}
					onstart={() => store.pushUndoState()}
					onchange={(nextValue) => {
						store.updateWatermarkSettings({ inset: nextValue });
					}}
				/>
			</div>
		</section>
	{/if}
</div>
