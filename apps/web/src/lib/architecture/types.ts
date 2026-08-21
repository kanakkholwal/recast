import type { DocHeading } from "$lib/docs/headings";

export type { DocHeading };

/** Mirrors the `architecture` branch of the frontmatter schema in `docvia.config.ts`. */
export type ArchitectureStatus = "production" | "beta" | "planned";

export type ArchitectureDomain =
	| "capture"
	| "editor"
	| "render"
	| "pipeline"
	| "platform"
	| "cloud"
	| "agent";

/** One subsystem page: its prose lives in markdown, these facts in frontmatter. */
export interface ArchitectureMeta {
	slug: string;
	url: string;
	title: string;
	description: string;
	/** One sentence a reader can stop at. Renders as the page lede. */
	summary: string;
	/** Reading order across the whole set, not within a domain. */
	position: number;
	status: ArchitectureStatus;
	domain: ArchitectureDomain;
	inputs: string[];
	outputs: string[];
	/** Real paths into the repo, deepest-first. */
	entrypoints: string[];
	/** Rules the subsystem breaks at its peril. The highest-value field here. */
	invariants: string[];
	/** Derived by docvia from the markdown, not written by hand. */
	headings: DocHeading[];
}

/** A page plus its compiled markdown tree. */
export interface ArchitectureDoc {
	meta: ArchitectureMeta;
	content: unknown;
}

/** Display order of the domain sections on the index. Follows the product spine. */
export const DOMAIN_ORDER: readonly ArchitectureDomain[] = [
	"platform",
	"capture",
	"editor",
	"render",
	"pipeline",
	"cloud",
	"agent",
];

export const DOMAIN_LABEL: Record<ArchitectureDomain, string> = {
	capture: "Capture",
	editor: "Editing",
	render: "Rendering",
	pipeline: "Pipelines",
	platform: "Platform",
	cloud: "Cloud",
	agent: "Agents",
};

/**
 * Hue per domain, following the landing page's Record → Polish → Share spine.
 * Platform and agents are neutral: they are not a step in it.
 */
export const DOMAIN_ACCENT: Record<
	ArchitectureDomain,
	"tangerine" | "lavender" | "green" | "neutral"
> = {
	capture: "tangerine",
	editor: "lavender",
	render: "lavender",
	pipeline: "green",
	cloud: "green",
	platform: "neutral",
	agent: "neutral",
};

export const STATUS_LABEL: Record<ArchitectureStatus, string> = {
	production: "In production",
	beta: "Beta",
	planned: "Planned",
};
