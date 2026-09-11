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

/**
 * Whether the NATIVE export can shape every visible text annotation itself. All or none, so a frame never
 * mixes two shapers; an empty family counts as missing, since the face it falls back to may not exist.
 */
export async function engineCanShapeText(
	annotations: Annotation[],
	resolveNative: (family: string, weight: number) => Promise<object | null>,
): Promise<boolean> {
	const texts = annotations.filter((a) => !a.hidden && a.kind.kind === "text");
	if (texts.length === 0) return false;
	if (texts.some((a) => a.kind.kind === "text" && !a.kind.fontFamily.trim())) return false;
	const found = await Promise.all(
		textAnnotationFaces(annotations).map(({ family, weight }) => resolveNative(family, weight)),
	);
	return found.every((face) => face !== null);
}
