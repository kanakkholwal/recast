import type { Annotation } from "../editor/render-state";

/** A family and weight some text on screen asks for. */
export interface FaceRequest {
	family: string;
	weight: number;
}

/**
 * The distinct faces the visible text annotations need, so a host can try to
 * supply them all before handing the engine the job. Hidden ones are excluded:
 * they draw nothing, and loading a font for them would stall the swap on text
 * nobody can see.
 */
export function textAnnotationFaces(annotations: Annotation[]): FaceRequest[] {
	const seen = new Map<string, FaceRequest>();
	for (const a of annotations) {
		if (a.hidden || a.kind.kind !== "text") continue;
		const family = a.kind.fontFamily.trim();
		if (!family) continue;
		const weight = a.kind.fontWeight;
		seen.set(`${family}:${weight}`, { family, weight });
	}
	return [...seen.values()];
}
