/**
 * Bridges CSS design tokens into canvas-usable colours.
 *
 * A 2D canvas can't read CSS custom properties, so the selection chrome used to
 * hardcode a raw blue that didn't even match the design system. Here we resolve
 * the real tokens once through a probe element (which the browser resolves
 * against the `:root` cascade) and normalise them to an sRGB triplet via a 1x1
 * canvas readback. That last step matters because engines serialise computed
 * `oklch()` values inconsistently, but the canvas parser always yields sRGB.
 *
 * The result is cached and refreshed when the theme changes.
 */

export interface SelectionPalette {
	/** Solid primary: selection outline and handle border. */
	accent: string;
	/** Primary at low alpha: soft outer ring around the selection. */
	accentRing: string;
	/** Primary at mid alpha: hover flash and snap guides. */
	accentMuted: string;
	/** Handle interior fill (background token). */
	surface: string;
	/** Badge text colour (primary-foreground). */
	onAccent: string;
	/** Resolved monospace family for the size badge. */
	monoFamily: string;
}

const FALLBACK_TRIPLET = "59, 130, 246";

let probe: HTMLElement | null = null;
let readCtx: CanvasRenderingContext2D | null = null;

function ensureProbe(): HTMLElement {
	if (probe?.isConnected) return probe;
	probe = document.createElement("span");
	probe.setAttribute("aria-hidden", "true");
	probe.style.cssText =
		"position:absolute;left:-9999px;top:-9999px;width:0;height:0;pointer-events:none;";
	document.body.appendChild(probe);
	return probe;
}

/** Resolve a CSS colour expression (e.g. `var(--primary)`) to an `"r, g, b"`
 *  triplet, normalised through a canvas so `oklch()` becomes sRGB. */
function resolveTriplet(expr: string): string {
	if (typeof document === "undefined") return FALLBACK_TRIPLET;
	const p = ensureProbe();
	p.style.color = expr;
	const resolved = getComputedStyle(p).color;
	if (!readCtx) {
		const c = document.createElement("canvas");
		readCtx = c.getContext("2d", { willReadFrequently: true });
	}
	if (!readCtx) return FALLBACK_TRIPLET;
	readCtx.clearRect(0, 0, 1, 1);
	readCtx.fillStyle = "#000";
	readCtx.fillStyle = resolved;
	readCtx.fillRect(0, 0, 1, 1);
	const [r, g, b] = readCtx.getImageData(0, 0, 1, 1).data;
	return `${r}, ${g}, ${b}`;
}

/** Resolve a token (e.g. `var(--primary)`) to an opaque `rgb()` string for
 *  baking into annotation content: a concrete colour the export can render. */
export function resolveTokenRgb(expr: string): string {
	return `rgb(${resolveTriplet(expr)})`;
}

/** Resolve a token to an `rgba()` string at the given alpha. */
export function resolveTokenRgba(expr: string, alpha: number): string {
	return `rgba(${resolveTriplet(expr)}, ${alpha})`;
}

function resolveFamily(expr: string): string {
	const p = ensureProbe();
	p.style.fontFamily = expr;
	return getComputedStyle(p).fontFamily || "monospace";
}

let cache: SelectionPalette | null = null;
let observer: MutationObserver | null = null;

function compute(): SelectionPalette {
	const primary = resolveTriplet("var(--primary)");
	return {
		accent: `rgb(${primary})`,
		accentRing: `rgba(${primary}, 0.35)`,
		accentMuted: `rgba(${primary}, 0.7)`,
		surface: `rgb(${resolveTriplet("var(--background)")})`,
		onAccent: `rgb(${resolveTriplet("var(--primary-foreground)")})`,
		monoFamily: resolveFamily("var(--font-mono)"),
	};
}

function watchTheme(): void {
	if (observer || typeof MutationObserver === "undefined") return;
	observer = new MutationObserver(() => {
		// Invalidate; recomputed lazily on next access so we never resolve mid-frame.
		cache = null;
	});
	observer.observe(document.documentElement, {
		attributes: true,
		attributeFilter: ["class", "style", "data-theme"],
	});
}

/** Cached selection palette, recomputed after a theme change. */
export function selectionPalette(): SelectionPalette {
	if (!cache) {
		cache = compute();
		watchTheme();
	}
	return cache;
}

/** Tear down the probe + observer. Call when the overlay unmounts. */
export function disposeCanvasTokens(): void {
	observer?.disconnect();
	observer = null;
	probe?.remove();
	probe = null;
	readCtx = null;
	cache = null;
}
