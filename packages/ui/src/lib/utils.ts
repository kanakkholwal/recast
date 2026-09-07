import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
	return twMerge(clsx(inputs));
}

export {
	BLOCK_BASE,
	BLOCK_HOVER,
	CRAFT_EASE,
	CRAFT_OVERLAY_ANIMATION,
	CRAFT_OVERLAY_BACKDROP_ANIMATION,
	CRAFT_TRANSITION,
	EASE_OUT,
	GLASS_PANEL,
	INVISIBLE_UI,
} from "./craft-utils";

// biome-ignore lint/suspicious/noExplicitAny: `any` is the probe that makes this conditional type match any child/children shape.
export type WithoutChild<T> = T extends { child?: any } ? Omit<T, "child"> : T;
// biome-ignore lint/suspicious/noExplicitAny: `any` is the probe that makes this conditional type match any child/children shape.
export type WithoutChildren<T> = T extends { children?: any } ? Omit<T, "children"> : T;
export type WithoutChildrenOrChild<T> = WithoutChildren<WithoutChild<T>>;
export type WithElementRef<T, U extends HTMLElement = HTMLElement> = T & { ref?: U | null };
