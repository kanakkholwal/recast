// RFC 8628 user codes are random ASCII. The convention is to show them split
// in half with a dash for readability; the plugin tolerates either form on the
// wire (POST /device/approve strips dashes before lookup).
export function formatUserCode(code: string | null | undefined): string {
	if (!code) return "";
	const clean = code.replace(/-/g, "").toUpperCase();
	if (clean.length <= 4) return clean;
	const half = Math.floor(clean.length / 2);
	return `${clean.slice(0, half)}-${clean.slice(half)}`;
}

// Sanitize manual entry down to the character set the plugin accepts before
// we round-trip it through the /device navigation.
export function normalizeUserCode(code: string): string {
	return code.trim().toUpperCase().replace(/[^A-Z0-9-]/g, "");
}
