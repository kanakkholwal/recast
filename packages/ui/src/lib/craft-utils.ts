/**
 * Craft.do Design System Utilities
 * 
 * Centralizing the "Invisible UI" logic and materiality tokens for consistency.
 */

export const CRAFT_TRANSITION = "transition-all duration-200 ease-in-out";

/**
 * The "Invisible UI" pattern:
 * Hide by default, show on parent group-hover or within focus.
 */
export const INVISIBLE_UI = "opacity-0 group-hover:opacity-100 focus-within:opacity-100 transition-opacity duration-200";

/**
 * Materiality presets
 */
export const GLASS_PANEL = "bg-white/70 dark:bg-black/70 backdrop-blur-md border border-white/20 dark:border-white/10 shadow-craft-floating";

export const BLOCK_BASE = "p-6 md:p-8 rounded-3xl bg-card transition-all duration-200";
export const BLOCK_HOVER = "hover:scale-[1.005] hover:bg-card/80 active:scale-[0.995]";
