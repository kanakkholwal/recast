/**
 * Card chrome shared by the library pages.
 *
 * These stay class strings rather than a `<LibraryCard>` component because the
 * container carries `animate:morph`, and Svelte only honours `animate:` on a
 * direct child of a keyed `{#each}` — wrapping it in a component silently kills
 * the grid↔list morph.
 */

export type LibraryView = "grid" | "list";

export const GRID_CLASS =
	"grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 2xl:grid-cols-6";

/** Wrapper for the results, laid out per view. */
export function listClass(view: LibraryView): string {
	return view === "grid" ? `grid gap-3.5 ${GRID_CLASS}` : "flex flex-col gap-1.5";
}

/** The card container itself. `selected` uses the neutral dark ring language. */
export function cardShellClass(view: LibraryView, selected: boolean): string {
	return [
		"group/card relative flex overflow-hidden border outline-none transition-[transform,background-color,border-color,box-shadow] duration-200 ease-out",
		view === "grid"
			? "flex-col rounded-2xl motion-safe:hover:-translate-y-0.5 motion-safe:active:scale-[0.99]"
			: "flex-row items-center gap-3 rounded-xl p-1.5 motion-safe:active:scale-[0.995]",
		selected
			? "border-foreground/40 bg-card ring-1 ring-inset ring-foreground/20"
			: "border-border/50 bg-card shadow-(--shadow-craft-inset) hover:border-border/80 hover:shadow-craft-md",
	].join(" ");
}

/** Thumbnail frame: fixed aspect in both views, full width only in the grid. */
export function thumbFrameClass(view: LibraryView): string {
	return [
		"relative shrink-0 overflow-hidden bg-muted/40",
		view === "grid" ? "aspect-video w-full" : "aspect-video w-22 rounded-md",
	].join(" ");
}

/** The card's click target, laid over everything but the actions menu. */
export const CARD_OVERLAY_CLASS =
	"absolute inset-0 z-10 cursor-pointer rounded-[inherit] focus:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/60";

/** Selection tick shown in the thumbnail corner while in select mode. */
export function selectTickClass(selected: boolean): string {
	return [
		"flex size-5 items-center justify-center rounded-md border backdrop-blur-md transition-all",
		selected
			? "border-foreground bg-foreground text-background"
			: "border-border/70 bg-background/80",
	].join(" ");
}

/** Position for the actions menu, above the click overlay in both views. */
export function cardActionsClass(view: LibraryView): string {
	return view === "grid" ? "absolute right-2 top-2 z-20" : "relative z-20 shrink-0 pr-1";
}
