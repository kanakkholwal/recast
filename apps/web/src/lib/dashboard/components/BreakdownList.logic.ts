/** Turn a 2-letter ISO country code into its flag emoji (regional-indicator
 *  pair). Returns "" for the sentinel buckets ("??", "__other") so callers
 *  can fall back to a neutral glyph. */
export function flagEmoji(code: string): string {
	if (!/^[a-z]{2}$/i.test(code)) return "";
	const base = 0x1f1e6;
	const cc = code.toUpperCase();
	return String.fromCodePoint(base + cc.charCodeAt(0) - 65, base + cc.charCodeAt(1) - 65);
}
