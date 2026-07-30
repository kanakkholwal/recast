<script lang="ts">
import { focusWindow } from "./focus-panel.logic";

// Two 0..1 sliders are a poor control for one point, so the pad is the primary
// affordance and the sliders below stay for typed precision. aria-hidden because
// those sliders already expose both values and ARIA has no two-axis slider role.
interface Props {
	centerX: number;
	centerY: number;
	scale: number;
	/** Frame aspect (width / height), so a portrait recording reads correctly. */
	aspect?: number;
	onstart?: () => void;
	onchange: (x: number, y: number) => void;
}

let { centerX, centerY, scale, aspect = 16 / 9, onstart, onchange }: Props = $props();

let el: HTMLDivElement | null = $state(null);
let dragging = $state(false);

const win = $derived(focusWindow(centerX, centerY, scale));
const pct = (v: number) => `${(v * 100).toFixed(3)}%`;

function pick(e: PointerEvent) {
	if (!el) return;
	const r = el.getBoundingClientRect();
	// Two decimals to match the sliders' step, so dragging and typing agree.
	const snap = (v: number) => Math.round(Math.min(1, Math.max(0, v)) * 100) / 100;
	onchange(snap((e.clientX - r.left) / r.width), snap((e.clientY - r.top) / r.height));
}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	bind:this={el}
	aria-hidden="true"
	class="relative w-full cursor-crosshair touch-none overflow-hidden rounded-md border border-border/60 bg-background/60"
	style="aspect-ratio: {aspect}"
	onpointerdown={(e) => {
		e.preventDefault();
		el?.setPointerCapture(e.pointerId);
		dragging = true;
		onstart?.();
		pick(e);
	}}
	onpointermove={(e) => {
		if (dragging) pick(e);
	}}
	onpointerup={() => (dragging = false)}
	onpointercancel={() => (dragging = false)}
	onlostpointercapture={() => (dragging = false)}
>
	<div class="pointer-events-none absolute inset-y-0 left-1/3 w-px bg-foreground/10"></div>
	<div class="pointer-events-none absolute inset-y-0 left-2/3 w-px bg-foreground/10"></div>
	<div class="pointer-events-none absolute inset-x-0 top-1/3 h-px bg-foreground/10"></div>
	<div class="pointer-events-none absolute inset-x-0 top-2/3 h-px bg-foreground/10"></div>

	<!-- What the viewer actually sees at this scale. Flush to an edge rather than
	     centred on the point, because both renderers pin the focus point. -->
	<div
		class="pointer-events-none absolute rounded-sm border border-primary/70 bg-primary/10 transition-[left,top,width,height] duration-150"
		style="left: {pct(win.left)}; top: {pct(win.top)}; width: {pct(win.size)}; height: {pct(
			win.size,
		)}"
	></div>

	<div
		class="pointer-events-none absolute size-2 -translate-x-1/2 -translate-y-1/2 rounded-full bg-primary ring-2 ring-background"
		style="left: {pct(centerX)}; top: {pct(centerY)}"
	></div>
</div>
