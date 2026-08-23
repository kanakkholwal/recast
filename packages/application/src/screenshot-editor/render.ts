import type { Frame, ImageFilters, ImageStyle, Shadow, TextShadow, Transform3D } from "./types";

/** The wrapper background for a style-frame preset. Values mirror the clone's
 * `styleConfig` (glass/outline are alpha-tinted; borders are solid). */
export function styleFrameBackground(style: ImageStyle): string {
	switch (style.preset) {
		case "glass-light":
			return `rgba(255, 255, 255, ${style.opacity})`;
		case "glass-dark":
			return `rgba(0, 0, 0, ${style.opacity})`;
		case "outline":
			return `rgba(255, 255, 255, ${style.opacity})`;
		case "border-light":
			return "rgb(255, 255, 255)";
		case "border-dark":
			return "rgb(26, 26, 26)";
		default:
			return "transparent";
	}
}

/** Compose color adjustments into a CSS `filter`, or "none" when all neutral.
 * Order mirrors the clone so exported output matches the preview. */
export function filtersCss(f: ImageFilters): string {
	const parts: string[] = [];
	if (f.brightness !== 100) parts.push(`brightness(${f.brightness}%)`);
	if (f.contrast !== 100) parts.push(`contrast(${f.contrast}%)`);
	if (f.saturate !== 100) parts.push(`saturate(${f.saturate}%)`);
	if (f.grayscale !== 0) parts.push(`grayscale(${f.grayscale}%)`);
	if (f.sepia !== 0) parts.push(`sepia(${f.sepia}%)`);
	if (f.hueRotate !== 0) parts.push(`hue-rotate(${f.hueRotate}deg)`);
	if (f.invert !== 0) parts.push(`invert(${f.invert}%)`);
	if (f.blur !== 0) parts.push(`blur(${f.blur}px)`);
	return parts.length ? parts.join(" ") : "none";
}

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

/** Compose a text overlay's shadow into a CSS `text-shadow`, or "none". */
export function textShadowCss(s: TextShadow): string {
	if (!s.enabled) return "none";
	return `${s.offsetX}px ${s.offsetY}px ${s.blur}px ${s.color}`;
}

/** Compose a frame border into a CSS `border`, or "none" when width is 0. */
export function borderCss(border: Frame["border"]): string {
	return border.width > 0 ? `${border.width}px solid ${border.color}` : "none";
}

/** Compose the 3D transform into a CSS `transform`. `translateX/Y` are percents
 * of the element's own box (matches the reference `perspective3D` translate). */
export function transformCss(t: Transform3D): string {
	const tx = t.translateX ?? 0;
	const ty = t.translateY ?? 0;
	const translate = tx !== 0 || ty !== 0 ? `translate(${tx}%, ${ty}%) ` : "";
	return `${translate}rotateX(${t.rotateX}deg) rotateY(${t.rotateY}deg) rotateZ(${t.rotateZ}deg) scale(${t.scale})`;
}
