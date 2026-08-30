// RFC 8628 codes are random ASCII, shown split with a dash; the plugin strips dashes before lookup, so either form works.
export function formatUserCode(code: string | null | undefined): string {
	if (!code) return "";
	const clean = code.replace(/-/g, "").toUpperCase();
	if (clean.length <= 4) return clean;
	const half = Math.floor(clean.length / 2);
	return `${clean.slice(0, half)}-${clean.slice(half)}`;
}

// Sanitize manual entry to the character set the plugin accepts before round-tripping through /device.
export function normalizeUserCode(code: string): string {
	return code
		.trim()
		.toUpperCase()
		.replace(/[^A-Z0-9-]/g, "");
}
