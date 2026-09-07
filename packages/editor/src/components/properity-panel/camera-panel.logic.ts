/**
 * CameraPanel helpers: human label for a position preset, and the inline CSS
 * that places a preset chip's dot where the bubble will land in the frame.
 */

import type { CameraLayout } from "../../lib/editor/render-state";
import { DEFAULT_SPLIT_FRACTION, LAYOUT_LABELS } from "../../lib/timeline/camera-clip-layout";
import type { CameraCapture } from "../../lib/wire-types";
import type { CameraPositionPreset } from "../../stores/editor-store.svelte";

export interface CameraAvailability {
	/** Whether the overlay controls can be used at all. */
	editable: boolean;
	title: string;
	description: string;
}

/**
 * What the panel should say, given how the camera was captured and whether the
 * track actually resolved to a file.
 *
 * The file's absence alone can't carry this: "the camera was off" and "this
 * project predates camera capture" both look like a missing path, and telling
 * someone they forgot a toggle that didn't exist when they recorded is wrong.
 * A recorded-but-missing track is its own case again — that's a broken project,
 * not a choice, and it should never read as "no camera".
 */
export function cameraAvailability(capture: CameraCapture, hasFile: boolean): CameraAvailability {
	if (capture === "separate") {
		if (hasFile) {
			return {
				editable: true,
				title: "Camera",
				description: "Recorded on its own track and composited when you export.",
			};
		}
		return {
			editable: false,
			title: "Camera track missing",
			description:
				"This project recorded a camera, but its track can't be found. Re-open the recording, or export without the overlay.",
		};
	}
	if (capture === "failed") {
		return {
			editable: false,
			title: "Camera didn't record",
			description:
				"The camera was switched on for this recording, but no track came through — usually the webcam being in use by another app, or camera permission being denied.",
		};
	}
	if (capture === "legacy") {
		return {
			editable: false,
			title: "No camera track",
			description:
				"This recording predates face-camera capture, so there's nothing to overlay. New recordings can capture the camera on its own track.",
		};
	}
	return {
		editable: false,
		title: "No camera track",
		description:
			"No camera was recorded for this project. Turn the camera on before recording to overlay it here.",
	};
}

/** Title-case a preset id ("top-left" → "Top Left") for aria-label/readouts. */
export function labelFor(preset: CameraPositionPreset): string {
	return preset
		.split("-")
		.map((part) => part[0].toUpperCase() + part.slice(1))
		.join(" ");
}

/**
 * Position the dot inside a preset chip so the chip reads as a miniature frame
 * with the bubble placed where it will land. Returns an inline `style` string.
 */
export function dotStyleFor(preset: CameraPositionPreset): string {
	if (preset === "custom") return "left:50%;top:50%;transform:translate(-50%,-50%);";
	// Detect each axis by token: the ids mix row-col and col-row conventions, so a positional split misplaces the dot.
	const tokens = preset.split("-");
	const col = tokens.includes("left") ? "left" : tokens.includes("right") ? "right" : "center";
	const row = tokens.includes("top") ? "top" : tokens.includes("bottom") ? "bottom" : "center";
	let xPart = "";
	let yPart = "";
	let translateX = "";
	let translateY = "";
	if (col === "left") xPart = "left:18%;";
	else if (col === "right") xPart = "right:18%;";
	else {
		xPart = "left:50%;";
		translateX = "translateX(-50%)";
	}
	if (row === "top") yPart = "top:18%;";
	else if (row === "bottom") yPart = "bottom:18%;";
	else {
		yPart = "top:50%;";
		translateY = "translateY(-50%)";
	}
	const transform =
		translateX && translateY
			? `transform:${translateX} ${translateY};`
			: translateX || translateY
				? `transform:${translateX || translateY};`
				: "";
	return xPart + yPart + transform;
}

/** Which of the panel's control groups the current arrangement actually uses. */
export interface CameraControls {
	/** The bubble's own geometry: position, size, shape and shadow. */
	bubble: boolean;
	/** What moves the bubble: per-cut positions and grow-on-zoom. */
	motion: boolean;
	/** Pointer dodging, which also needs a pointer to dodge. */
	dodge: boolean;
	/** Why the bubble controls do not apply, or null when they do. */
	reason: string | null;
	/** Why dodging does not apply. It has a second cause the others do not. */
	dodgeReason: string | null;
}

/** Why each layout ignores the bubble controls, in the panel's own words. */
const IGNORES_THE_BUBBLE: Partial<Record<CameraLayout["kind"], string>> = {
	splitH: "This clip splits the frame, so the camera fills its half.",
	splitV: "This clip splits the frame, so the camera fills its half.",
	screenOnly: "This clip hides the camera.",
	cameraOnly: "This clip gives the camera the whole frame.",
};

/**
 * A layout decides which controls mean anything. Offering all of them for every
 * arrangement let a split's Position grid and Width field write state that
 * nothing read, which reads as a broken control rather than an unused one.
 */
export function cameraControls(layout: CameraLayout, cursorEnabled: boolean): CameraControls {
	const reason = IGNORES_THE_BUBBLE[layout.kind] ?? null;
	const bubble = reason === null;
	return {
		bubble,
		motion: bubble,
		dodge: bubble && cursorEnabled,
		reason,
		dodgeReason: bubble && !cursorEnabled ? "Turn the pointer on to dodge it." : reason,
	};
}

/** Picker rows for the layout select, named the same as the timeline's clips. */
export const CAMERA_LAYOUT_OPTIONS = LAYOUT_LABELS.map((l) => ({
	value: l.kind,
	label: l.label,
}));

/** What the two halves are called, which differs by split axis. */
export function splitSideOptions(kind: "splitH" | "splitV") {
	return kind === "splitH"
		? [
				{ value: "start", label: "Left" },
				{ value: "end", label: "Right" },
			]
		: [
				{ value: "start", label: "Top" },
				{ value: "end", label: "Bottom" },
			];
}

/**
 * The layout to store when the picker changes kind. The picker carries only a
 * kind, so a split needs its share and side filled in: carried over from the
 * layout being replaced where it had them, so flipping between the two split
 * axes keeps the framing the user just set.
 */
export function layoutForKind(current: CameraLayout, kind: CameraLayout["kind"]): CameraLayout {
	if (kind !== "splitH" && kind !== "splitV") return { kind } as CameraLayout;
	const wasSplit = current.kind === "splitH" || current.kind === "splitV";
	return {
		kind,
		fraction: wasSplit ? current.fraction : DEFAULT_SPLIT_FRACTION,
		side: wasSplit ? current.side : "start",
	};
}
