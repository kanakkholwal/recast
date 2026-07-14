<script lang="ts">
	import type { Snippet } from "svelte";
	import { cn } from "@recast/ui/utils";

	// Full-bleed showcase panel — a softly-tinted rounded card that holds
	// the section's visual + text composition. Each section picks a tone so
	// the landing reads as a sequence of distinct showcases rather than
	// one continuous scroller.
	//
	// Tones are deliberately near-neutral (very low opacity backgrounds)
	// so they support the brand's primary (lime) and the dark canvas
	// without competing with either. The visual weight comes from the
	// rounded card shape, generous padding, and structure, not from
	// saturated backgrounds.
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
		padding?: "default" | "tight" | "loose";
		class?: string;
	} = $props();

	// Tones are a hint of color over the page bg, not a solid wash.
	// Light mode sits at 30% so the wash is felt, not seen; dark mode
	// pulls to 12% because the dark canvas already absorbs the hue.
	const toneClass: Record<typeof tone, string> = {
		blue: "bg-blue-50/30 dark:bg-blue-950/12",
		green: "bg-emerald-50/30 dark:bg-emerald-950/12",
		yellow: "bg-amber-50/30 dark:bg-amber-950/12",
		violet: "bg-violet-50/30 dark:bg-violet-950/12",
		neutral: "bg-foreground/[0.02] dark:bg-foreground/[0.03]",
	};

	const paddingClass: Record<typeof padding, string> = {
		tight: "p-6 sm:p-9 md:p-12",
		default: "p-6 sm:p-12 md:p-16 lg:p-20",
		loose: "p-8 sm:p-14 md:p-20 lg:p-24",
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