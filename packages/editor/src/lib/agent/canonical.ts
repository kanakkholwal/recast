/**
 * Null-vs-omitted normalisation for comparing our own state against a backend
 * echo. Rust serializes `Option` fields with `skip_serializing_if`, so the echo
 * OMITS keys the frontend writes as explicit `null`. A raw `JSON.stringify`
 * guard therefore never matches its own write and the listener reconciles
 * forever — the recording panel shipped exactly that freeze once.
 */

/** Recursively drop null/undefined entries and sort object keys. */
export function canonicalize(value: unknown): unknown {
	if (Array.isArray(value)) return value.map(canonicalize);
	if (value === null || typeof value !== "object") return value;

	const source = value as Record<string, unknown>;
	const out: Record<string, unknown> = {};
	for (const key of Object.keys(source).sort()) {
		const entry = source[key];
		if (entry === null || entry === undefined) continue;
		out[key] = canonicalize(entry);
	}
	return out;
}

/** Stable string for equality checks. Never use raw JSON.stringify instead. */
export function canonicalKey(value: unknown): string {
	return JSON.stringify(canonicalize(value));
}

/** True when two render states are the same edit, ignoring null-vs-omitted. */
export function sameRenderState(a: unknown, b: unknown): boolean {
	return canonicalKey(a) === canonicalKey(b);
}
