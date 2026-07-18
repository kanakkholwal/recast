// Local `cn` helper. Inlined to keep `@recast/icons` standalone (no
// workspace dependency) — call sites can still use Tailwind class-merging
// without pulling in the rest of `@recast/ui`. Mirrors the lucide-style
// API: takes a class string plus optional conditionals.
import type { ClassValue } from "clsx";

export type { ClassValue };

export function cn(...inputs: ClassValue[]): string {
	return inputs.filter(Boolean).join(" ");
}
