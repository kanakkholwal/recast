import type { Frame, Shadow, Transform3D } from "./types";

/** Convert a `#rgb`/`#rrggbb` hex plus an alpha into a CSS `rgba()`. Falls back
 * to the input untouched if it isn't a hex (already `rgb()`/named), so callers
 * can pass any CSS color. */
export function hexWithAlpha(color: string, alpha: number): string {
  const m = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(color.trim());
  if (!m) return color;
  let hex = m[1];
  if (hex.length === 3) hex = hex[0] + hex[0] + hex[1] + hex[1] + hex[2] + hex[2];
  const r = parseInt(hex.slice(0, 2), 16);
  const g = parseInt(hex.slice(2, 4), 16);
  const b = parseInt(hex.slice(4, 6), 16);
  const a = Math.max(0, Math.min(1, alpha));
  return `rgba(${r}, ${g}, ${b}, ${a})`;
}

/** Compose a Shadow into a CSS `box-shadow`, or "none" when invisible. */
export function shadowCss(shadow: Shadow): string {
  if (shadow.opacity <= 0) return "none";
  const { x, y, blur, spread } = shadow;
  return `${x}px ${y}px ${blur}px ${spread}px ${hexWithAlpha(shadow.color, shadow.opacity)}`;
}

/** Compose a frame border into a CSS `border`, or "none" when width is 0. */
export function borderCss(border: Frame["border"]): string {
  return border.width > 0 ? `${border.width}px solid ${border.color}` : "none";
}

/** Compose the 3D transform into a CSS `transform`. */
export function transformCss(t: Transform3D): string {
  return `rotateX(${t.rotateX}deg) rotateY(${t.rotateY}deg) rotateZ(${t.rotateZ}deg) scale(${t.scale})`;
}
