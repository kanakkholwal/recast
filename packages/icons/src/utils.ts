// Inlined so `@recast/icons` stays standalone: call sites get Tailwind class-merging without depending on `@recast/ui`.
import type { ClassValue } from "clsx";

export type { ClassValue };

export function cn(...inputs: ClassValue[]): string {
	return inputs.filter(Boolean).join(" ");
}
