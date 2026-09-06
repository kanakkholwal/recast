<script lang="ts">
import { cn } from "@recast/ui/utils";
import type { Snippet } from "svelte";

// A tonal paper surface or a hairline card, never a tinted wash: a coloured surface competes with the feature tags.
let {
	children,
	tone = "paper",
	padding = "default",
	class: className = "",
}: {
	children: Snippet;
	/** `paper` = tonal band, `card` = white + hairline, `bare` = no surface. */
	tone?: "paper" | "card" | "bare";
	padding?: "default" | "tight" | "loose";
	class?: string;
} = $props();

const toneClass = {
	paper: "bg-paper",
	card: "bg-card border border-border-low",
	bare: "",
} as const;

const paddingClass = {
	tight: "p-6 sm:p-8",
	default: "p-6 sm:p-10 md:p-12",
	loose: "p-8 sm:p-12 md:p-16",
} as const;
</script>

<div
	class={cn(
		"relative overflow-hidden rounded-2xl",
		toneClass[tone],
		paddingClass[padding],
		className,
	)}
>
	{@render children()}
</div>
