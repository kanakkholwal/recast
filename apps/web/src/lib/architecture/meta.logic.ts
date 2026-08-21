import { docHeadings } from "$lib/docs/headings";
import {
	type ArchitectureDomain,
	type ArchitectureMeta,
	type ArchitectureStatus,
	DOMAIN_ORDER,
} from "./types";

const STATUSES: readonly ArchitectureStatus[] = ["production", "beta", "planned"];

function strings(value: unknown): string[] {
	return Array.isArray(value) ? value.map(String).filter((entry) => entry.length > 0) : [];
}

function status(value: unknown): ArchitectureStatus {
	return STATUSES.includes(value as ArchitectureStatus)
		? (value as ArchitectureStatus)
		: "production";
}

function domain(value: unknown): ArchitectureDomain {
	return DOMAIN_ORDER.includes(value as ArchitectureDomain)
		? (value as ArchitectureDomain)
		: "platform";
}

/**
 * Normalize one page's frontmatter.
 *
 * The docvia build already validated it against the schema, so this coerces
 * rather than re-validates, but it never trusts a field to be present, because
 * the compiled `meta` is plain JSON by the time it reaches a route.
 */
export function toArchitectureMeta(
	slug: string,
	url: string,
	data: Record<string, unknown>,
): ArchitectureMeta {
	return {
		slug,
		url,
		title: String(data.title ?? slug),
		description: String(data.description ?? ""),
		summary: String(data.summary ?? data.description ?? ""),
		position: Number.isFinite(Number(data.position))
			? Number(data.position)
			: Number.MAX_SAFE_INTEGER,
		status: status(data.status),
		domain: domain(data.domain),
		inputs: strings(data.inputs),
		outputs: strings(data.outputs),
		entrypoints: strings(data.entrypoints),
		invariants: strings(data.invariants),
		headings: docHeadings(data.headings),
	};
}

/** Reading order. Ties break on title so the list never reorders between builds. */
export function sortDocs(docs: readonly ArchitectureMeta[]): ArchitectureMeta[] {
	return [...docs].sort((a, b) => a.position - b.position || a.title.localeCompare(b.title));
}

export interface DomainSection {
	domain: ArchitectureDomain;
	docs: ArchitectureMeta[];
}

/** Group into index sections, dropping domains nothing is filed under. */
export function groupByDomain(docs: readonly ArchitectureMeta[]): DomainSection[] {
	const sorted = sortDocs(docs);
	return DOMAIN_ORDER.map((domainName) => ({
		domain: domainName,
		docs: sorted.filter((doc) => doc.domain === domainName),
	})).filter((section) => section.docs.length > 0);
}

export interface Neighbours {
	previous: ArchitectureMeta | null;
	next: ArchitectureMeta | null;
}

/** Previous and next in reading order, for the footer pager. */
export function neighbours(docs: readonly ArchitectureMeta[], slug: string): Neighbours {
	const sorted = sortDocs(docs);
	const index = sorted.findIndex((doc) => doc.slug === slug);
	if (index < 0) return { previous: null, next: null };
	return {
		previous: sorted[index - 1] ?? null,
		next: sorted[index + 1] ?? null,
	};
}

/**
 * The last path segment of an entrypoint, for the compact chip label.
 *
 * Directory entrypoints keep their trailing slash so `commands/export/` does not
 * read as a file called `export`.
 */
export function entrypointLabel(path: string): string {
	const trimmed = path.replace(/\/+$/, "");
	const leaf = trimmed.slice(trimmed.lastIndexOf("/") + 1);
	return path.endsWith("/") ? `${leaf}/` : leaf;
}
