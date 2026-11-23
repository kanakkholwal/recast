/**
 * Render any thrown value as ONE log line that can never come out empty.
 *
 * Both matter, and both were learned the hard way while chasing a prerender
 * failure that reached CI as a bare "GET /blog → 500" with no cause:
 *
 *  - Single line. Vercel's build-log capture keeps the first line of a write and
 *    drops the rest, so a newline-separated stack vanishes. Newlines are folded
 *    to " ⏎ " instead.
 *  - `||`, not `??`. An `Error` whose `stack` is the empty string (bundled and
 *    minified server output can produce one) is not nullish, so `stack ?? message`
 *    yields "" and prints nothing at all. Every field is tried in turn, and a
 *    value that still renders as nothing falls back to naming its own type.
 */
export function describeError(error: unknown): string {
	const flatten = (s: string) => s.replace(/\s*\n\s*/g, " ⏎ ").trim();

	if (error instanceof Error) {
		const parts = [flatten(error.stack || "") || flatten(`${error.name}: ${error.message}`)];
		// Zod and better-auth wrap the real failure in `cause`, so without this the log stops where the detail begins.
		if (error.cause !== undefined) parts.push(`cause: ${describeError(error.cause)}`);
		// Anything the class hangs off the error beyond the standard fields, such as `issues` or `code`.
		const extras = Object.getOwnPropertyNames(error).filter(
			(k) => !["name", "message", "stack", "cause"].includes(k),
		);
		for (const key of extras) {
			parts.push(`${key}=${safeJson((error as unknown as Record<string, unknown>)[key])}`);
		}
		return parts.filter(Boolean).join(" | ");
	}

	const rendered = flatten(safeJson(error));
	return rendered && rendered !== "{}" ? rendered : `<non-error ${typeof error}: ${String(error)}>`;
}

function safeJson(value: unknown): string {
	try {
		return JSON.stringify(value) ?? String(value);
	} catch {
		return String(value);
	}
}
