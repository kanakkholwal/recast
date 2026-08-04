/**
 * Cursor sprite library. SVGs so we can recolour/resample at any DPI without
 * pixel assets. Add a variant by dropping an `.svg` into `./sprites/` and
 * referencing its bare filename via `sprite()` in `CURSOR_STYLES`.
 *
 * Every sprite is authored at 64×64 with its click hotspot at `hotspot`
 * (sprite-space px); the overlay translates by `-hotspot` so the tip lands on
 * the captured pointer position. Drop-shadow filters use unique IDs to avoid
 * clashing when multiple sprites share the DOM.
 */

import type { CursorStyleId } from "$lib/stores/editor-store.svelte";

export interface CursorStyle {
  id: CursorStyleId;
  label: string;
  /** Short blurb shown under the swatch in the panel. */
  description: string;
  /** Authored at 64×64 with the click hotspot at `hotspot`. */
  svg: string;
  /** Optional pressed-state sprite swapped in while the captured cursor
   *  is mid-click. When omitted the rest sprite is reused. */
  pressedSvg?: string;
  /** Optional right-click sprite. Falls back to {@link pressedSvg} → {@link svg}. */
  rightPressedSvg?: string;
  /** Optional drag sprite (button held while moving). Falls back to
   *  {@link pressedSvg} → {@link svg}. */
  dragSvg?: string;
  hotspot: { x: number; y: number };
  pressedHotspot?: { x: number; y: number };
  rightPressedHotspot?: { x: number; y: number };
  dragHotspot?: { x: number; y: number };
}

// Raw SVG strings inlined at build time (no runtime fs access), keyed by path.
const spriteModules = import.meta.glob<string>("./sprites/*.svg", {
  query: "?raw",
  import: "default",
  eager: true,
});

/** Resolve a sprite by bare filename. Throws at module init on a missing file
 *  so a typo surfaces immediately instead of rendering an empty cursor. */
function sprite(name: string): string {
  const svg = spriteModules[`./sprites/${name}.svg`];
  if (!svg) {
    throw new Error(
      `cursor sprite "./sprites/${name}.svg" not found. Available: ${Object.keys(
        spriteModules,
      ).join(", ")}`,
    );
  }
  return svg;
}

export const CURSOR_STYLES: CursorStyle[] = [
  {
    id: "dot",
    label: "Soft dot",
    description: "Default cursor, used in preview and export.",
    // `dot` is drawn by the WebGL2 shader; this SVG is only the picker swatch.
    svg: sprite("dot"),
    hotspot: { x: 32, y: 32 },
  },
  {
    // Apple cursor set: rest=arrow, press=link hand, rightPress=context, drag=grab.
    // See ./sprites/CREDITS.md for attribution.
    id: "macos-system",
    label: "macOS System",
    description: "Apple cursor set: arrow, link hand, grab, and right-click states.",
    svg: sprite("system-macos-arrow"),
    pressedSvg: sprite("system-macos-pointer"),
    rightPressedSvg: sprite("system-macos-context"),
    dragSvg: sprite("system-macos-grab"),
    hotspot: { x: 18.6, y: 10.7 },
    pressedHotspot: { x: 24.7, y: 11.3 },
    rightPressedHotspot: { x: 18, y: 17 },
    dragHotspot: { x: 32, y: 32 },
  },
  {
    // Windows cursor set. No distinct right-click cursor, so rightPress falls
    // back to press → rest; drag uses the 4-way move cursor.
    // See ./sprites/CREDITS.md for attribution.
    id: "windows-system",
    label: "Windows System",
    description: "Windows cursor set: arrow, link hand, and move (drag) states.",
    svg: sprite("system-windows-arrow"),
    pressedSvg: sprite("system-windows-hand"),
    dragSvg: sprite("system-windows-move"),
    hotspot: { x: 19.3, y: 12.4 },
    pressedHotspot: { x: 28.5, y: 11 },
    dragHotspot: { x: 32, y: 32 },
  },
];

export function getCursorStyle(id: CursorStyleId): CursorStyle {
  return CURSOR_STYLES.find((s) => s.id === id) ?? CURSOR_STYLES[0];
}

export type CursorStyleState = "rest" | "press" | "rightPress" | "drag";

/** Resolve the sprite SVG for a state, falling back rightPress/drag → press →
 *  rest so a style that only ships rest (+ press) never renders blank. */
export function cursorStyleSvg(
  style: CursorStyle,
  state: CursorStyleState,
): string {
  switch (state) {
    case "rightPress":
      return style.rightPressedSvg ?? style.pressedSvg ?? style.svg;
    case "drag":
      return style.dragSvg ?? style.pressedSvg ?? style.svg;
    case "press":
      return style.pressedSvg ?? style.svg;
    default:
      return style.svg;
  }
}

export function cursorStyleHotspot(
  id: CursorStyleId,
  state: CursorStyleState = "rest",
): { x: number; y: number } {
  const style = getCursorStyle(id);
  switch (state) {
    case "rightPress":
      return style.rightPressedHotspot ?? style.pressedHotspot ?? style.hotspot;
    case "drag":
      return style.dragHotspot ?? style.pressedHotspot ?? style.hotspot;
    case "press":
      return style.pressedHotspot ?? style.hotspot;
    default:
      return style.hotspot;
  }
}

/** Cached `data:image/svg+xml,…` URLs (one per id+state) so the `<img>`
 *  element in the overlay layer doesn't re-encode on every frame. */
const dataUrlCache = new Map<string, string>();
export function cursorStyleDataUrl(
  id: CursorStyleId,
  state: CursorStyleState = "rest",
): string {
  const key = `${id}:${state}`;
  const cached = dataUrlCache.get(key);
  if (cached) return cached;
  const style = getCursorStyle(id);
  const svg = cursorStyleSvg(style, state);
  const url =
    "data:image/svg+xml;utf8," +
    encodeURIComponent(svg.trim().replace(/\n\s*/g, " "));
  dataUrlCache.set(key, url);
  return url;
}
