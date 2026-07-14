<script lang="ts">
	import type { Snippet } from "svelte";
	import { cn } from "@recast/ui/utils";

	// Full-bleed showcase panel — a softly-tinted rounded card that holds
	// the section's visual + text composition. Each section picks a tone so
	// the landing reads as a sequence of distinct showcases rather than
	// one continuous scroller.
	//
	// Tones are very low opacity (the panel is more "atmosphere" than
	// "block of color") so they support the brand's primary (lime) and
	// the dark canvas without fighting either. The visual weight comes
	// from the rounded corners, generous padding, and structure — not
	// from saturated backgrounds.
	let {
		children,
		tone = "neutral",
		padding = "default",
		class: className = "",
	}: {
		children: Snippet;
		/** Section accent color. One per section so the page reads as a sequence. */
		tone?: "blue" | "green" | "yellow" | "violet" | "neutral";
		/** Internal padding scale. "default" for most showcases, "tight" for narrow content. */
		padding?: "default" | "tight";
		class?: string;
	} = $props();

	const toneClass: Record<typeof tone, string> = {
		// Very low-opacity tints — closer to a wash than a block. Tinted
		// backdrop barely shows in dark mode, where the canvas is already
		// near-black, and stays subtle in light mode.
		blue: "bg-blue-50/40 dark:bg-blue-950/20",
		green: "bg-emerald-50/40 dark:bg-emerald-950/20",
		yellow: "bg-amber-50/40 dark:bg-amber-950/20",
		violet: "bg-violet-50/40 dark:bg-violet-950/20",
		neutral: "bg-foreground/[0.025] dark:bg-foreground/[0.03]",
	};

	const paddingClass: Record<typeof padding, string> = {
		default: "p-6 sm:p-10 md:p-14",
		// Tighter padding for narrower showcases where the content is already
		// contained (e.g. centered headline + small card grid).
		tight: "p-6 sm:p-8 md:p-10",
	};
</script>

<div
	class={cn(
		"relative overflow-hidden rounded-[2.25rem]",
		toneClass[tone],
		paddingClass[padding],
		className,
	)}
>
	{@render children()}
</div>