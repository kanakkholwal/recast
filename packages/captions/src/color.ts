/** `#rrggbb` (or `#rrggbbaa`) + a 0..1 factor -> `rgba(...)`. The factor
 *  multiplies any alpha already on the hex. Non-hex input is returned as-is so
 *  a CSS keyword or var() passes through untouched. */
export function withAlpha(color: string, factor: number): string {
	const f = Math.max(0, Math.min(1, factor));
	const c = color.trim();
	const hex8 = /^#?([0-9a-fA-F]{6})([0-9a-fA-F]{2})$/.exec(c);
	if (hex8) {
		const v = parseInt(hex8[1], 16);
		const a = (parseInt(hex8[2], 16) / 255) * f;
		return `rgba(${(v >> 16) & 0xff},${(v >> 8) & 0xff},${v & 0xff},${a.toFixed(3)})`;
	}
	const hex6 = /^#?([0-9a-fA-F]{6})$/.exec(c);
	if (hex6) {
		const v = parseInt(hex6[1], 16);
		return `rgba(${(v >> 16) & 0xff},${(v >> 8) & 0xff},${v & 0xff},${f.toFixed(3)})`;
	}
	return c;
}
