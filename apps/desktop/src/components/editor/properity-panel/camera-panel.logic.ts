/**
 * CameraPanel helpers: human label for a position preset, and the inline CSS
 * that places a preset chip's dot where the bubble will land in the frame.
 */

import type { CameraPositionPreset } from "$lib/stores/editor-store.svelte";

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
