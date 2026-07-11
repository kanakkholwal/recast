/** AudioPanel helpers + data: fade presets, the dB readout, and the SVG gain-envelope path. */

export interface FadePreset {
	label: string;
	in: number;
	out: number;
}

export const FADE_PRESETS: FadePreset[] = [
	{ label: "None", in: 0, out: 0 },
	{ label: "Subtle", in: 0.25, out: 0.25 },
	{ label: "Smooth", in: 0.5, out: 1.0 },
	{ label: "Cinematic", in: 1.0, out: 2.0 },
];

/** Volume readout zone. Above ~105% the export applies straight gain (can clip). */
export type VolumeZone = "muted" | "low" | "nominal" | "boost" | "hot";

export function volumeZone(muted: boolean, volume: number): VolumeZone {
	if (muted || volume <= 0) return "muted";
	if (volume < 70) return "low";
	if (volume <= 105) return "nominal";
	if (volume <= 150) return "boost";
	return "hot";
}

/** A preset matches when both fade lengths are within a step of the settings. */
export function isPresetActive(
	settings: { fadeIn: number; fadeOut: number },
	preset: FadePreset,
): boolean {
	return (
		Math.abs(settings.fadeIn - preset.in) < 0.01 &&
		Math.abs(settings.fadeOut - preset.out) < 0.01
	);
}

/** Label of the matching preset (drives the Segmented selection); "" = custom. */
export function activePreset(settings: {
	fadeIn: number;
	fadeOut: number;
}): string {
	return FADE_PRESETS.find((p) => isPresetActive(settings, p))?.label ?? "";
}

/**
 * dB-ish display: 20·log10(volume/100), so "0 dB" at 100%. Intentionally not the
 * ffmpeg `volume=` curve (a linear multiplier), because this matches user intuition.
 */
export function dbForVolume(v: number): string {
	if (v <= 0) return "−∞ dB";
	const db = 20 * Math.log10(v / 100);
	if (Math.abs(db) < 0.05) return "0.0 dB";
	return `${db > 0 ? "+" : ""}${db.toFixed(1)} dB`;
}

/**
 * SVG gain-envelope path over a 100×24 box, mirroring FFmpeg afade: linear ramp
 * 0→1 over `fadeIn`, hold, then 1→0 over `fadeOut`. Each fade capped at half the clip.
 */
export function envelopePath(
	fadeIn: number,
	fadeOut: number,
	clipDuration: number,
): string {
	const W = 100;
	const H = 24;
	const totalSecs = Math.max(0.01, clipDuration || 1);
	const fi = Math.max(0, Math.min(fadeIn, totalSecs * 0.5));
	const fo = Math.max(0, Math.min(fadeOut, totalSecs * 0.5));
	const xIn = (fi / totalSecs) * W;
	const xOut = W - (fo / totalSecs) * W;
	const yTop = 2;
	const yBottom = H - 2;
	return `M 0 ${yBottom} L ${xIn.toFixed(2)} ${yTop} L ${xOut.toFixed(2)} ${yTop} L ${W} ${yBottom}`;
}
