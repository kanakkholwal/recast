/**
 * Shared chrome for the player control row, so the transport, markup and view
 * clusters read as one bar instead of three separately-invented ones.
 */

export const BAR_GROUP =
	"flex items-center gap-0.5 rounded-lg bg-muted/60 p-0.5 ring-1 ring-inset ring-border/40";

export const BAR_BTN =
	"flex size-7 cursor-pointer items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground";

/** Raised pill: the bar's "on" state for view toggles and the play button. */
export const BAR_BTN_ON =
	"bg-card text-foreground shadow-(--shadow-craft-inset) ring-1 ring-inset ring-border/40";

export const BAR_BTN_DISABLED =
	"disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent disabled:hover:text-muted-foreground";
