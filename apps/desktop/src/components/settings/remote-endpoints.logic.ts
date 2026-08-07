/**
 * Pure helpers for the remote-transcription endpoints manager: form validation
 * (mirrors the Rust `remote::validate_endpoint` rules), id slugging, and payload
 * building. Kept side-effect-free so it's unit-tested without a webview.
 */

import type { RemoteAsrEndpoint } from "@recast/editor/lib/wire-types";

export interface EndpointForm {
	id: string;
	displayName: string;
	baseUrl: string;
	model: string;
	/** Comma/space separated in the form; parsed to a list on save. */
	languages: string;
}

export function emptyForm(): EndpointForm {
	return { id: "", displayName: "", baseUrl: "", model: "", languages: "" };
}

/** Build a path-safe slug from a display name (matches the Rust id rules:
 *  alphanumerics plus `-`, `_`, `.`). Capped to 64 chars. */
export function slugify(name: string): string {
	return name
		.toLowerCase()
		.trim()
		.replace(/[^a-z0-9._-]+/g, "-")
		.replace(/^[-.]+|[-.]+$/g, "")
		.slice(0, 64);
}

/** Same id rule as Rust `is_safe_ext_id`: non-empty slug, no path tricks. */
export function isValidId(id: string): boolean {
	return (
		id.length > 0 &&
		id.length <= 64 &&
		id !== "." &&
		id !== ".." &&
		!id.startsWith(".") &&
		/^[a-zA-Z0-9._-]+$/.test(id)
	);
}

/** Mirror of Rust `normalize_base_url`: absolute http(s) URL with a host. */
export function isValidBaseUrl(raw: string): boolean {
	const trimmed = raw.trim().replace(/\/+$/, "");
	if (!trimmed) return false;
	try {
		const u = new URL(trimmed);
		return (u.protocol === "http:" || u.protocol === "https:") && u.host.length > 0;
	} catch {
		return false;
	}
}

/** First validation error for a form, or `null` when it's ready to save. */
export function validateForm(f: EndpointForm): string | null {
	if (!f.displayName.trim()) return "Enter a name for this endpoint.";
	if (!isValidId(f.id)) return "Id must be letters, numbers, dashes, or dots.";
	if (!isValidBaseUrl(f.baseUrl)) {
		return "Base URL must be an absolute http(s) URL, e.g. http://127.0.0.1:1234/v1";
	}
	if (!f.model.trim()) return "Enter the model name the endpoint expects.";
	return null;
}

/** Split the languages field into a clean list (empty when none given). */
export function parseLanguages(raw: string): string[] {
	return raw
		.split(/[\s,]+/)
		.map((s) => s.trim())
		.filter(Boolean);
}

/** Turn a validated form into the IPC payload (base URL trailing slash stripped). */
export function toEndpoint(f: EndpointForm): RemoteAsrEndpoint {
	return {
		id: f.id.trim(),
		displayName: f.displayName.trim(),
		baseUrl: f.baseUrl.trim().replace(/\/+$/, ""),
		model: f.model.trim(),
		languages: parseLanguages(f.languages),
	};
}

/** Prefill a form from an existing endpoint for editing. */
export function formFromEndpoint(ep: RemoteAsrEndpoint): EndpointForm {
	return {
		id: ep.id,
		displayName: ep.displayName,
		baseUrl: ep.baseUrl,
		model: ep.model,
		languages: ep.languages.join(", "),
	};
}
