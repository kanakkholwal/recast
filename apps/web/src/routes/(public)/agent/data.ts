// Static data for /agent, one const per export so the bundler tree-shakes what the page doesn't use.

import {
	AiWand,
	Captions,
	Eye,
	FileBox,
	GitCommit,
	type IconComponent,
	ListChecks,
	MessageSquare,
	Play,
	Scissors,
	ShieldCheck,
	Target,
	VolumeX,
} from "@recast/icons";

// The real one-liner the page prints; keep it in sync with the recast-editing skill.
export const connect = {
	command: "claude mcp add recast -- recast mcp",
	clients: ["Claude Code", "Claude Desktop", "Cursor"],
};

// The three beats: see, propose, apply.
export const model = [
	{
		icon: Eye,
		title: "It sees the take",
		description:
			"Transcript on the output clock, frames pulled to disk, detected silences, the full timeline. The agent reads the recording before it touches it.",
		tags: ["Transcript", "Frames", "Silences"],
	},
	{
		icon: GitCommit,
		title: "It proposes on a branch",
		description:
			"Cuts, zooms, captions and annotations land on a branch, never on your project. The editor stays the source of truth.",
		tags: ["Branch-first", "Cuts and zooms", "Captions"],
	},
	{
		icon: ListChecks,
		title: "You review and apply",
		description:
			"A field-level diff of every change, a check pass that surfaces what the state hides, and a one-move undo. Apply it, or discard it.",
		tags: ["Diff", "Check", "Undo"],
	},
];

// Individual capabilities, grouped by the kicker tag.
export const capabilities: Array<{
	icon: IconComponent;
	tag: string;
	title: string;
	description: string;
}> = [
	{
		icon: MessageSquare,
		tag: "Perceive",
		title: "Read the transcript",
		description:
			"Words on the output clock, windowed, so the agent quotes the take instead of guessing.",
	},
	{
		icon: Play,
		tag: "Perceive",
		title: "Look at frames",
		description:
			"JPEG frames at evenly spaced output times, cuts skipped, written to disk for the agent to open.",
	},
	{
		icon: VolumeX,
		tag: "Perceive",
		title: "Find the silences",
		description: "Detected dead-air spans on the output clock, ready to cut.",
	},
	{
		icon: Scissors,
		tag: "Edit",
		title: "Remove silences",
		description: "Cut every detected silence onto a branch in one call.",
	},
	{
		icon: Target,
		tag: "Edit",
		title: "Add zooms",
		description: "Place a zoom at an output second. No keyframes.",
	},
	{
		icon: Captions,
		tag: "Edit",
		title: "Captions and annotations",
		description: "Arrows, text, blur and captions as typed ops, layered on the timeline.",
	},
	{
		icon: AiWand,
		tag: "Edit",
		title: "Raw ops escape hatch",
		description:
			"Any edit the schema allows, appended as typed ops. The full surface, not a walled subset.",
	},
	{
		icon: ListChecks,
		tag: "Review",
		title: "Field-level diff",
		description: "Dotted paths with before and after: exactly what a reviewer sees.",
	},
	{
		icon: ShieldCheck,
		tag: "Review",
		title: "Check before hand-off",
		description:
			"Findings you cannot read off the state: overlaps, gaps, edits that land off-frame.",
	},
	{
		icon: GitCommit,
		tag: "Review",
		title: "Truncate or discard",
		description:
			"Undo the tail of a branch, or drop the whole proposal. Nothing lands until you apply.",
	},
	{
		icon: FileBox,
		tag: "Document",
		title: "project.rcx",
		description: "The recording is a document: agent-legible, git-diffable, patchable op by op.",
	},
];

// Why it is safe to point an agent at your work.
export const guarantees = [
	{
		title: "Nothing writes without you",
		description: "No tool records, exports, or edits the project. The agent proposes; you apply.",
	},
	{
		title: "Stale reads are refused",
		description:
			"Every build is gated on the project hash. If the editor moved, the op is rejected, not merged blind.",
	},
	{
		title: "Retries never double-apply",
		description: "An idempotency key per attempt means a retried call lands once, or not at all.",
	},
	{
		title: "Stays on your machine",
		description: "The server is local stdio. Your recording never leaves the device to be edited.",
	},
];
