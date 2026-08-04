/**
 * CameraPanel helpers: human label for a position preset, and the inline CSS
 * that places a preset chip's dot where the bubble will land in the frame.
 */

import type { CameraPositionPreset } from "$lib/stores/editor-store.svelte";
import type { CameraCapture } from "$lib/ipc-types";

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
	// Detect each axis by token: the ids mix 'row-col' ('top-left') and 'col-row'
	// ('left-center') conventions, so a positional split mis-places the dot.
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
