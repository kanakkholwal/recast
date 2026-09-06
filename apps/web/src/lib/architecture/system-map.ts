/**
 * The one place the top-level data flow is described.
 *
 * This replaced the mermaid copy that used to open the architecture README: a
 * diagram nobody could click into, kept in sync with the document index by hand.
 * Nodes here carry the slug of the page that owns them, so the map is also the
 * navigation.
 */

/** Which phase of Record → Polish → Share a node belongs to. Drives its hue. */
export type MapPhase = "record" | "polish" | "share" | "artifact";

export interface MapNode {
	id: string;
	label: string;
	/** Where the work happens. Rendered as the node's second line. */
	runtime: "Rust" | "Browser" | "File" | "Cloud";
	phase: MapPhase;
	/** The architecture page that documents it, or null for a plain artifact. */
	slug: string | null;
	x: number;
	y: number;
}

export interface MapEdge {
	source: string;
	target: string;
	label?: string;
}

const COLUMN = 250;
const ROW = 92;

export const SYSTEM_NODES: readonly MapNode[] = [
	{
		id: "capture",
		label: "Screen capture",
		runtime: "Rust",
		phase: "record",
		slug: "recording-pipeline",
		x: 0,
		y: 0,
	},
	{
		id: "audio",
		label: "Mic + system audio",
		runtime: "Rust",
		phase: "record",
		slug: "recording-pipeline",
		x: 0,
		y: ROW,
	},
	{
		id: "cursor",
		label: "Cursor at 125 Hz",
		runtime: "Rust",
		phase: "record",
		slug: "recording-pipeline",
		x: 0,
		y: ROW * 2,
	},
	{
		id: "camera",
		label: "Camera",
		runtime: "Browser",
		phase: "record",
		slug: "recording-pipeline",
		x: 0,
		y: ROW * 3,
	},
	{
		id: "cli",
		label: "CLI + control socket",
		runtime: "Rust",
		phase: "record",
		slug: "cli-control-socket",
		x: 0,
		y: ROW * 4,
	},

	{
		id: "encoder",
		label: "H.264 encoder",
		runtime: "Rust",
		phase: "record",
		slug: "recording-pipeline",
		x: COLUMN,
		y: ROW * 0.5,
	},
	{
		id: "project",
		label: ".recast bundle",
		runtime: "File",
		phase: "artifact",
		slug: "state-project-format",
		x: COLUMN,
		y: ROW * 2.5,
	},

	{
		id: "store",
		label: "EditorStore",
		runtime: "Browser",
		phase: "polish",
		slug: "state-project-format",
		x: COLUMN * 2,
		y: ROW * 1.5,
	},
	{
		id: "timeline",
		label: "Timeline model",
		runtime: "Browser",
		phase: "polish",
		slug: "timeline-model",
		x: COLUMN * 2,
		y: ROW * 0.25,
	},
	{
		id: "captions",
		label: "Captions + ASR",
		runtime: "Rust",
		phase: "polish",
		slug: "captions-transcription",
		x: COLUMN * 2,
		y: ROW * 2.75,
	},
	{
		id: "branches",
		label: "Agent branches",
		runtime: "Rust",
		phase: "polish",
		slug: "agentic-edits-mcp",
		x: COLUMN * 2,
		y: ROW * 4,
	},

	{
		id: "decode",
		label: "MediaBunny decode",
		runtime: "Browser",
		phase: "polish",
		slug: "media-decode-workers",
		x: COLUMN * 3,
		y: ROW * 0.25,
	},
	{
		id: "core",
		label: "Engine (wgpu/wasm)",
		runtime: "Browser",
		phase: "polish",
		slug: "preview-engine",
		x: COLUMN * 3,
		y: ROW * 1.75,
	},
	{
		id: "ipc",
		label: "Tauri IPC",
		runtime: "Rust",
		phase: "artifact",
		slug: "ipc-tauri-boundary",
		x: COLUMN * 3,
		y: ROW * 3.25,
	},

	{
		id: "preview",
		label: "Live preview",
		runtime: "Browser",
		phase: "polish",
		slug: "preview-engine",
		x: COLUMN * 4,
		y: 0,
	},
	{
		id: "export",
		label: "Export encode",
		runtime: "Browser",
		phase: "share",
		slug: "export-pipeline",
		x: COLUMN * 4,
		y: ROW * 1.5,
	},
	{
		id: "mux",
		label: "FFmpeg mux",
		runtime: "Rust",
		phase: "share",
		slug: "export-pipeline",
		x: COLUMN * 4,
		y: ROW * 2.75,
	},

	{
		id: "file",
		label: "mp4 or gif",
		runtime: "File",
		phase: "artifact",
		slug: null,
		x: COLUMN * 5,
		y: ROW * 1.5,
	},
	{
		id: "cloud",
		label: "Share link",
		runtime: "Cloud",
		phase: "share",
		slug: "cloud-sharing-extensions",
		x: COLUMN * 5,
		y: ROW * 2.75,
	},
];

export const SYSTEM_EDGES: readonly MapEdge[] = [
	{ source: "capture", target: "encoder" },
	{ source: "audio", target: "project" },
	{ source: "cursor", target: "project" },
	{ source: "camera", target: "project" },
	{ source: "cli", target: "capture", label: "drives" },
	{ source: "encoder", target: "project", label: "recording.mp4" },

	{ source: "project", target: "store", label: "load" },
	{ source: "store", target: "project", label: "save" },
	{ source: "store", target: "timeline" },
	{ source: "captions", target: "store" },
	{ source: "branches", target: "project", label: "apply" },
	{ source: "cli", target: "branches" },

	{ source: "timeline", target: "core" },
	{ source: "store", target: "core", label: "snapshot" },
	{ source: "project", target: "decode" },
	{ source: "decode", target: "core", label: "frames" },
	{ source: "core", target: "preview" },
	{ source: "core", target: "export", label: "same passes" },
	{ source: "ipc", target: "mux" },
	{ source: "export", target: "mux", label: "video only" },
	{ source: "mux", target: "file" },
	{ source: "file", target: "cloud", label: "upload" },
];

/** Slugs the map links to, so a test can assert none of them 404. */
export function mappedSlugs(nodes: readonly MapNode[] = SYSTEM_NODES): string[] {
	return [
		...new Set(nodes.map((node) => node.slug).filter((slug): slug is string => slug !== null)),
	];
}

/** Ids referenced by an edge but never declared, which would render a dangling line. */
export function danglingEdges(
	nodes: readonly MapNode[] = SYSTEM_NODES,
	edges: readonly MapEdge[] = SYSTEM_EDGES,
): string[] {
	const ids = new Set(nodes.map((node) => node.id));
	const missing = edges.flatMap((edge) => [edge.source, edge.target].filter((id) => !ids.has(id)));
	return [...new Set(missing)];
}
