import type { ErrorContext, ScrubbedError } from "./types";

/**
 * Pure PII scrubbing for error payloads. No SDK import — runs identically on
 * JS errors and on `{ name, message, stack }` payloads forwarded from Rust, so
 * both surfaces redact the same way before anything leaves the machine.
 *
 * Applied in `core.captureError` before any provider sees the error.
 */

const ALLOWED_CONTEXT_KEYS = ["route", "command", "os", "app_version", "source", "phase"] as const;

// Order matters: paths and origins first (most specific), then identifiers.
const REDACTIONS: Array<[RegExp, string]> = [
	// Windows extended-length device paths: \\?\C:\... and UNC \\server\share
	[/\\\\\?\\[^\s"'`]+/g, "<path>"],
	// Windows user home: C:\Users\<name>\...  ->  C:\Users\<user>
	[/([A-Za-z]:\\Users\\)[^\\/\s"'`]+/g, "$1<user>"],
	// Unix home: /Users/<name> or /home/<name>  ->  /Users/<user>
	[/(\/(?:Users|home)\/)[^/\s"'`]+/g, "$1<user>"],
	// file:// , tauri:// , http(s):// origins  ->  scheme://<host> (drops host + path tail handled below)
	[/((?:file|tauri|https?):\/\/)[^\s/"'`)]+/g, "$1<host>"],
	// Bearer / Authorization tokens
	[/(bearer\s+)[A-Za-z0-9._\-]+/gi, "$1<redacted>"],
	[/(authorization["':\s]+)[A-Za-z0-9._\-\s]+/gi, "$1<redacted>"],
	// Email addresses
	[/[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/g, "<email>"],
	// Query-string values: ?k=v&k2=v2 -> ?k=<redacted>&k2=<redacted>
	[/([?&][^=\s&"'`]+=)[^&\s"'`]+/g, "$1<redacted>"],
	// UUIDs (often user/session ids)
	[/[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}/g, "<uuid>"],
];

/** Redact known PII shapes from a free-text string. Pure + idempotent-ish. */
export function redact(input: string): string {
	let out = input;
	for (const [pattern, replacement] of REDACTIONS) {
		out = out.replace(pattern, replacement);
	}
	return out;
}

/** Stable, low-cardinality fingerprint so the same error groups together. */
function fingerprint(name: string, message: string): string {
	// Normalize volatile bits (numbers, the redaction tokens) out of the message
	// so "failed after 1234ms" and "failed after 5678ms" share a fingerprint.
	const normalized = `${name}:${message}`
		.replace(/\d+/g, "#")
		.replace(/<[a-z]+>/g, "#")
		.toLowerCase();
	let hash = 5381;
	for (let i = 0; i < normalized.length; i++) {
		hash = (hash * 33) ^ normalized.charCodeAt(i);
	}
	return (hash >>> 0).toString(16);
}

/**
 * Turn an arbitrary thrown value (or a Rust-forwarded payload) into a scrubbed,
 * send-ready error. The context is allow-listed — only the keys in
 * `ALLOWED_CONTEXT_KEYS` survive, and each value is itself redacted.
 */
export function scrubError(err: unknown, ctx: ErrorContext = {}): ScrubbedError {
	let name = "Error";
	let message = "";
	let stack: string | undefined;

	if (err instanceof Error) {
		name = err.name || "Error";
		message = err.message || "";
		stack = err.stack || undefined;
	} else if (typeof err === "string") {
		message = err;
	} else if (err && typeof err === "object") {
		const e = err as Record<string, unknown>;
		name = typeof e.name === "string" ? e.name : "Error";
		message = typeof e.message === "string" ? e.message : safeStringify(e);
		stack = typeof e.stack === "string" ? e.stack : undefined;
	} else {
		message = String(err);
	}

	const cleanMessage = redact(message);
	const cleanStack = stack ? redact(stack) : undefined;

	const context: Record<string, string> = {};
	for (const key of ALLOWED_CONTEXT_KEYS) {
		const v = ctx[key];
		if (typeof v === "string" && v.length > 0) context[key] = redact(v);
	}

	return {
		name,
		message: cleanMessage,
		stack: cleanStack,
		fingerprint: fingerprint(name, cleanMessage),
		context,
	};
}

function safeStringify(value: unknown): string {
	try {
		return JSON.stringify(value);
	} catch {
		return "[unserializable error]";
	}
}
