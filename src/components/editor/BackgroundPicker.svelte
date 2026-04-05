<script lang="ts">
	import {
	  COLOR_PRESETS,
	  GRADIENT_PRESETS,
	  WALLPAPERS,
	  type BackgroundType,
	  type EditorStore,
	} from "$lib/stores/editor-store.svelte";
	import { Blend, ImageIcon, Palette, Sparkles } from "@lucide/svelte";
	import SliderControl from "./SliderControl.svelte";

	interface Props {
		store: EditorStore;
	}

	let { store }: Props = $props();



	const bgTabs: {
		type: BackgroundType;
		label: string;
		icon: typeof Sparkles;
	}[] = [
		{ type: "wallpaper", label: "Wallpaper", icon: Sparkles },
		{ type: "image", label: "Image", icon: ImageIcon },
		{ type: "color", label: "Color", icon: Palette },
		{ type: "gradient", label: "Gradient", icon: Blend },
	];



	function selectBackground(type: BackgroundType, value: string) {
		store.backgroundType = type;
		store.backgroundValue = value;
	}

	let blurValue = $state(40);
	let paddingValue = $state(32);

	// Sync from store on mount
	$effect(() => {
		blurValue = store.backgroundBlur;
		paddingValue = store.padding;
	});
</script>

<div class="flex flex-col gap-5 animate-in fade-in duration-300">
	<!-- Background Image Section -->
	<section>
		<h4
			class="mb-3 flex items-center gap-2 text-sm font-semibold text-foreground"
		>
			<Sparkles size={14} class="text-muted-foreground" />
			Background Image
		</h4>

		<!-- Type Tabs -->
		<div
			class="mb-3 flex gap-1 rounded-xl bg-muted/50 p-1 border border-border/50"
		>
			{#each bgTabs as tab}
				{@const Icon = tab.icon}
				<button
					onclick={() =>
						selectBackground(tab.type, store.backgroundValue)}
					class="flex flex-1 items-center justify-center gap-1.5 rounded-lg px-2 py-1.5 text-[11px] font-medium transition-all duration-200
						{store.backgroundType === tab.type
						? 'bg-background text-foreground shadow-sm'
						: 'text-muted-foreground hover:text-foreground'}"
				>
					<Icon size={12} />
					{tab.label}
				</button>
			{/each}
		</div>

		<!-- Wallpaper Content -->
		{#if store.backgroundType === "wallpaper"}
			<!-- Wallpaper Grid -->
			<div class="grid grid-cols-5 gap-1.5">
				{#each WALLPAPERS as wp, i}
					<button
						onclick={() => selectBackground("wallpaper", wp.src)}
						class="group relative aspect-square overflow-hidden rounded-lg border transition-all duration-200 animate-in fade-in zoom-in-95
							{store.backgroundValue === wp.src
							? 'border-primary ring-2 ring-primary/30 shadow-md'
							: 'border-border/50 hover:border-border hover:shadow-sm'}"
						style="animation-delay: {i * 30}ms"
						title={wp.label}
					>
						<img
							src={wp.src}
							alt={wp.label}
							class="h-full w-full object-cover transition-transform duration-300 group-hover:scale-110"
							draggable="false"
						/>
					</button>
				{/each}

			</div>
		{:else if store.backgroundType === "color"}
			<!-- Color Grid -->
			<div class="grid grid-cols-6 gap-2">
				{#each COLOR_PRESETS as color, i}
					<button
						onclick={() => selectBackground("color", color)}
						class="group aspect-square rounded-lg border-2 transition-all duration-200 animate-in fade-in zoom-in-95
							{store.backgroundValue === color
							? 'border-primary ring-2 ring-primary/30 scale-110'
							: 'border-border/30 hover:border-border hover:scale-105'}"
						style="background-color: {color}; animation-delay: {i *
							20}ms"
						title={color}
					></button>
				{/each}
			</div>
			<!-- Custom color input -->
			<div class="mt-3 flex items-center gap-2">
				<input
					type="color"
					value={store.backgroundValue.startsWith("#")
						? store.backgroundValue
						: "#000000"}
					oninput={(e) =>
						selectBackground(
							"color",
							(e.target as HTMLInputElement).value,
						)}
					class="h-8 w-8 cursor-pointer rounded-md border border-border bg-transparent"
				/>
				<span class="text-xs font-mono text-muted-foreground"
					>{store.backgroundValue}</span
				>
			</div>
		{:else if store.backgroundType === "gradient"}
			<!-- Gradient Grid -->
			<div class="grid grid-cols-3 gap-2">
				{#each GRADIENT_PRESETS as grad, i}
					<button
						onclick={() => selectBackground("gradient", grad.value)}
						class="group h-14 rounded-lg border-2 transition-all duration-200 animate-in fade-in zoom-in-95
							{store.backgroundValue === grad.value
							? 'border-primary ring-2 ring-primary/30 scale-105'
							: 'border-border/30 hover:border-border hover:scale-[1.03]'}"
						style="background: {grad.value}; animation-delay: {i *
							30}ms"
						title={grad.label}
					></button>
				{/each}
			</div>
		{:else}
			<!-- Image upload placeholder -->
			<div
				class="flex flex-col items-center justify-center rounded-xl border-2 border-dashed border-border/50 py-8 text-center transition-colors hover:border-border"
			>
				<ImageIcon size={24} class="mb-2 text-muted-foreground/50" />
				<p class="text-xs text-muted-foreground">
					Click to upload a custom image
				</p>
			</div>
		{/if}
	</section>

	<!-- Background Blur -->
	<section>
		<SliderControl
			label="Background Blur"
			bind:value={blurValue}
			min={0}
			max={100}
			step={1}
			unit="%"
			onchange={(v) => {
				store.backgroundBlur = v;
			}}
		>
			{#snippet icon()}
				<Blend size={12} />
			{/snippet}
		</SliderControl>
	</section>

	<!-- Padding -->
	<section>
		<SliderControl
			label="Padding"
			bind:value={paddingValue}
			min={0}
			max={100}
			step={1}
			unit="px"
			onchange={(v) => {
				store.padding = v;
			}}
		>
			{#snippet icon()}
				<svg
					width="12"
					height="12"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
				>
					<rect x="3" y="3" width="18" height="18" rx="2" />
					<rect x="7" y="7" width="10" height="10" rx="1" />
				</svg>
			{/snippet}
		</SliderControl>
	</section>
</div>
