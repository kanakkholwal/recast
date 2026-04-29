<script lang="ts" module>
	export type ColorValue = string; // CSS color (hex/#rrggbb[aa] or rgba())

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

	function clamp01(v: number): number {
		return Math.max(0, Math.min(1, v));
	}

	export function parseColor(
		input: string,
	): { r: number; g: number; b: number; a: number } | null {
		const v = input.trim();
		if (!v) return null;
		// #rgb / #rgba
		let m = v.match(/^#([0-9a-f])([0-9a-f])([0-9a-f])([0-9a-f])?$/i);
		if (m) {
			const r = parseInt(m[1] + m[1], 16);
			const g = parseInt(m[2] + m[2], 16);
			const b = parseInt(m[3] + m[3], 16);
			const a = m[4] ? parseInt(m[4] + m[4], 16) / 255 : 1;
			return { r, g, b, a };
		}
		// #rrggbb / #rrggbbaa
		m = v.match(/^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})?$/i);
		if (m) {
			const r = parseInt(m[1], 16);
			const g = parseInt(m[2], 16);
			const b = parseInt(m[3], 16);
			const a = m[4] ? parseInt(m[4], 16) / 255 : 1;
			return { r, g, b, a };
		}
		// rgb(...) / rgba(...)
		m = v.match(/^rgba?\(([^)]+)\)$/i);
		if (m) {
			const parts = m[1]
				.split(/[,\s]+/)
				.filter(Boolean)
				.map((p) => p.trim());
			if (parts.length < 3) return null;
			const r = Math.round(+parts[0]);
			const g = Math.round(+parts[1]);
			const b = Math.round(+parts[2]);
			const a = parts[3] !== undefined ? clamp01(+parts[3]) : 1;
			if ([r, g, b].some(Number.isNaN)) return null;
			return { r, g, b, a };
		}
		return null;
	}

	export function formatHex(r: number, g: number, b: number, a: number): string {
		const rh = r.toString(16).padStart(2, "0");
		const gh = g.toString(16).padStart(2, "0");
		const bh = b.toString(16).padStart(2, "0");
		if (a >= 1) return `#${rh}${gh}${bh}`;
		const ah = Math.round(clamp01(a) * 255)
			.toString(16)
			.padStart(2, "0");
		return `#${rh}${gh}${bh}${ah}`;
	}

	export function rgbToHsl(
		r: number,
		g: number,
		b: number,
	): { h: number; s: number; l: number; a: number } {
		const rn = r / 255;
		const gn = g / 255;
		const bn = b / 255;
		const max = Math.max(rn, gn, bn);
		const min = Math.min(rn, gn, bn);
		const l = (max + min) / 2;
		let h = 0;
		let s = 0;
		if (max !== min) {
			const d = max - min;
			s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
			switch (max) {
				case rn:
					h = (gn - bn) / d + (gn < bn ? 6 : 0);
					break;
				case gn:
					h = (bn - rn) / d + 2;
					break;
				case bn:
					h = (rn - gn) / d + 4;
					break;
			}
			h *= 60;
		}
		return { h: Math.round(h), s: Math.round(s * 100), l: Math.round(l * 100), a: 1 };
	}

	export function hslToRgb(
		h: number,
		s: number,
		l: number,
	): { r: number; g: number; b: number } {
		const hn = h / 360;
		const sn = s / 100;
		const ln = l / 100;
		if (sn === 0) {
			const v = Math.round(ln * 255);
			return { r: v, g: v, b: v };
		}
		const q = ln < 0.5 ? ln * (1 + sn) : ln + sn - ln * sn;
		const p = 2 * ln - q;
		const hueToRgb = (t: number) => {
			let tn = t;
			if (tn < 0) tn += 1;
			if (tn > 1) tn -= 1;
			if (tn < 1 / 6) return p + (q - p) * 6 * tn;
			if (tn < 1 / 2) return q;
			if (tn < 2 / 3) return p + (q - p) * (2 / 3 - tn) * 6;
			return p;
		};
		return {
			r: Math.round(hueToRgb(hn + 1 / 3) * 255),
			g: Math.round(hueToRgb(hn) * 255),
			b: Math.round(hueToRgb(hn - 1 / 3) * 255),
		};
	}
</script>

<script lang="ts">
	import { cn } from "@recast/ui/utils";
	import { Pipette } from "@lucide/svelte";

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
		// `EyeDropper` is a Chromium-only API; gated above so we don't surface
		// the button on browsers/webviews that lack it.
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
		class="relative h-32 w-full cursor-crosshair rounded-md border border-border"
		style:background={`linear-gradient(to top, #000, transparent), linear-gradient(to right, #fff, hsl(${hue} 100% 50%))`}
		onpointerdown={handleSlPointer}
	>
		<span
			class="pointer-events-none absolute size-3 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow-craft-sm ring-1 ring-black/40"
			style:left={`${markerPos.x}%`}
			style:top={`${markerPos.y}%`}
			style:background={`hsl(${hue} ${sat}% ${light}%)`}
		></span>
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
