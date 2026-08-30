<script lang="ts" module>
import { formatHex, hslToRgb, parseColor, rgbToHsl, type ColorValue } from "./color-picker.logic";

export type { ColorValue };

export interface ColorPickerProps {
	value: ColorValue;
	oncommit: (next: ColorValue) => void;
	/** Optional list of swatches shown above the picker. */
	swatches?: string[];
	/** Optional list of recently-used colors. */
	recents?: string[];
	/** Show an alpha slider. Default true. */
	allowAlpha?: boolean;
	class?: string;
}

const DEFAULT_SWATCHES: string[] = [
	"#3b82f6",
	"#ef4444",
	"#22c55e",
	"#f59e0b",
	"#a855f7",
	"#ec4899",
	"#06b6d4",
	"#ffffff",
];
</script>

<script lang="ts">
	import { cn } from "@recast/ui/utils";
	import { Pipette } from "@recast/icons";

	let {
		value = "#3b82f6",
		oncommit,
		swatches = DEFAULT_SWATCHES,
		recents = [],
		allowAlpha = true,
		class: className,
	}: ColorPickerProps = $props();

	// Internal HSL representation; commits as hex (or 8-digit hex when alpha < 1).
	let hue = $state(0);
	let sat = $state(0);
	let light = $state(50);
	let alpha = $state(1);
	let hexInput = $state("");

	// Re-sync internal HSL whenever `value` changes externally.
	$effect(() => {
		const parsed = parseColor(value);
		if (!parsed) return;
		const { h, s, l } = rgbToHsl(parsed.r, parsed.g, parsed.b);
		hue = h;
		sat = s;
		light = l;
		alpha = parsed.a;
		hexInput = formatHex(parsed.r, parsed.g, parsed.b, parsed.a);
	});

	const currentRgb = $derived(hslToRgb(hue, sat, light));
	const currentCss = $derived(formatHex(currentRgb.r, currentRgb.g, currentRgb.b, alpha));

	function commit() {
		oncommit(currentCss);
	}

	function handleHueInput(e: Event) {
		hue = +(e.currentTarget as HTMLInputElement).value;
		commit();
	}

	function handleAlphaInput(e: Event) {
		alpha = +(e.currentTarget as HTMLInputElement).value;
		commit();
	}

	function handleSatInput(e: Event) {
		sat = +(e.currentTarget as HTMLInputElement).value;
		commit();
	}

	function handleLightInput(e: Event) {
		light = +(e.currentTarget as HTMLInputElement).value;
		commit();
	}

	function handleSlPointer(e: PointerEvent) {
		const target = e.currentTarget as HTMLElement;
		(target as Element).setPointerCapture(e.pointerId);
		updateSlFromEvent(e, target);
		const move = (ev: PointerEvent) => updateSlFromEvent(ev, target);
		const up = (ev: PointerEvent) => {
			target.removeEventListener("pointermove", move);
			target.removeEventListener("pointerup", up);
			target.removeEventListener("pointercancel", up);
			(target as Element).releasePointerCapture(ev.pointerId);
		};
		target.addEventListener("pointermove", move);
		target.addEventListener("pointerup", up);
		target.addEventListener("pointercancel", up);
	}

	function updateSlFromEvent(e: PointerEvent, target: HTMLElement) {
		const rect = target.getBoundingClientRect();
		const x = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
		const y = Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height));
		const hsvS = x;
		const hsvV = 1 - y;
		const l = hsvV * (1 - hsvS / 2);
		const s = l === 0 || l === 1 ? 0 : (hsvV - l) / Math.min(l, 1 - l);
		sat = Math.round(s * 100);
		light = Math.round(l * 100);
		commit();
	}

	function selectSwatch(c: string) {
		const parsed = parseColor(c);
		if (!parsed) return;
		hexInput = formatHex(parsed.r, parsed.g, parsed.b, parsed.a);
		oncommit(c);
	}

	function commitHexInput() {
		const parsed = parseColor(hexInput);
		if (!parsed) return;
		oncommit(formatHex(parsed.r, parsed.g, parsed.b, parsed.a));
	}

	const hasEyedropper = typeof window !== "undefined" && "EyeDropper" in window;

	async function pickWithEyedropper() {
		// `EyeDropper` is Chromium-only and gated above, so the button never appears where it is missing.
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		const Picker: any = (window as any).EyeDropper;
		if (!Picker) return;
		try {
			const result = await new Picker().open();
			if (result?.sRGBHex) oncommit(result.sRGBHex);
		} catch {
			// User dismissed — silent.
		}
	}

	const markerPos = $derived.by(() => {
		const l = light / 100;
		const s = sat / 100;
		const v = l + s * Math.min(l, 1 - l);
		const sv = v === 0 ? 0 : 2 * (1 - l / v);
		return { x: sv * 100, y: (1 - v) * 100 };
	});
</script>

<div class={cn("flex w-64 flex-col gap-3 p-3", className)} data-slot="color-picker">
	{#if swatches.length}
		<div class="flex flex-wrap gap-1.5">
			{#each swatches as swatch (swatch)}
				<button
					type="button"
					onclick={() => selectSwatch(swatch)}
					aria-label={`Pick ${swatch}`}
					class="size-5 rounded-full border border-border ring-offset-background transition-transform hover:scale-110 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
					style:background={swatch}
				></button>
			{/each}
		</div>
	{/if}

	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="relative h-32 w-full cursor-crosshair rounded-md border border-border focus-within:ring-2 focus-within:ring-ring"
		style:background={`linear-gradient(to top, #000, transparent), linear-gradient(to right, #fff, hsl(${hue} 100% 50%))`}
		onpointerdown={handleSlPointer}
	>
		<span
			class="pointer-events-none absolute size-3 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow-craft-sm ring-1 ring-black/40"
			style:left={`${markerPos.x}%`}
			style:top={`${markerPos.y}%`}
			style:background={`hsl(${hue} ${sat}% ${light}%)`}
		></span>
		<!-- ARIA has no two-axis slider role, so the field's axes are exposed as
		     two ordinary sliders — same mechanism as Hue below, and arrows /
		     Home / End / PageUp come for free. -->
		<input
			type="range"
			min="0"
			max="100"
			step="1"
			value={sat}
			oninput={handleSatInput}
			aria-label="Saturation"
			class="sr-only"
		/>
		<input
			type="range"
			min="0"
			max="100"
			step="1"
			value={light}
			oninput={handleLightInput}
			aria-label="Lightness"
			class="sr-only"
		/>
	</div>

	<input
		type="range"
		min="0"
		max="360"
		step="1"
		value={hue}
		oninput={handleHueInput}
		aria-label="Hue"
		class="h-3 w-full appearance-none rounded-full"
		style="background: linear-gradient(to right, hsl(0 100% 50%), hsl(60 100% 50%), hsl(120 100% 50%), hsl(180 100% 50%), hsl(240 100% 50%), hsl(300 100% 50%), hsl(360 100% 50%));"
	/>

	{#if allowAlpha}
		<input
			type="range"
			min="0"
			max="1"
			step="0.01"
			value={alpha}
			oninput={handleAlphaInput}
			aria-label="Alpha"
			class="h-3 w-full appearance-none rounded-full"
			style={`background: linear-gradient(to right, transparent, hsl(${hue} ${sat}% ${light}%)), repeating-conic-gradient(#cbd5e1 0% 25%, transparent 0% 50%) 0 0/8px 8px;`}
		/>
	{/if}

	<div class="flex items-center gap-2">
		<span
			class="size-7 shrink-0 rounded-md border border-border"
			style:background={currentCss}
			aria-hidden="true"
		></span>
		<input
			type="text"
			bind:value={hexInput}
			onblur={commitHexInput}
			onkeydown={(e) => {
				if (e.key === "Enter") commitHexInput();
			}}
			spellcheck="false"
			class="h-7 w-full rounded-md border border-border bg-background px-2 font-mono text-[11px] text-foreground outline-none focus:border-primary/60 focus:ring-1 focus:ring-primary/30"
			placeholder="#3b82f6"
		/>
		{#if hasEyedropper}
			<button
				type="button"
				onclick={pickWithEyedropper}
				class="grid size-7 shrink-0 place-items-center rounded-md border border-border text-muted-foreground transition-colors hover:bg-muted/50 hover:text-foreground"
				title="Pick from screen"
				aria-label="Eyedropper"
			>
				<Pipette size={12} />
			</button>
		{/if}
	</div>

	{#if recents.length}
		<div class="space-y-1">
			<p class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
				Recent
			</p>
			<div class="flex flex-wrap gap-1.5">
				{#each recents as r (r)}
					<button
						type="button"
						onclick={() => selectSwatch(r)}
						aria-label={`Use recent ${r}`}
						class="size-5 rounded-full border border-border transition-transform hover:scale-110"
						style:background={r}
					></button>
				{/each}
			</div>
		</div>
	{/if}
</div>
