<!--
	Colorful reaction glyphs (flat emoji-reaction style, Facebook-ish) rendered
	as self-contained inline SVG — no external asset, no CDN, works offline and
	under the share page's strict CSP. Keyed by the stable reaction `id` from
	`$lib/share/reactions`, so swapping the artwork later is a one-file change
	with zero data migration. Decorative: the accessible label lives on the
	button that wraps this.
-->
<script lang="ts">
	import type { ReactionId } from "$lib/share/reactions";
	import { cn } from "@recast/ui/utils";

	let { id, class: className = "" }: { id: ReactionId; class?: string } = $props();
</script>

<svg viewBox="0 0 24 24" class={cn("shrink-0", className)} aria-hidden="true">
	{#if id === "like"}
		<circle cx="12" cy="12" r="10" fill="#3B8AFF" />
		<path
			fill="#fff"
			d="M6.4 11h1.7v6.2H6.4a.8.8 0 0 1-.8-.8v-4.6a.8.8 0 0 1 .8-.8Zm2.7-.2c1.5-.7 2.5-2 2.9-3.7.1-.5.2-1 .2-1.5 0-.8 1-1.1 1.6-.6.5.4.8 1.1.8 1.8V8.8h3c.9 0 1.6.9 1.4 1.8l-1 4.4a1.9 1.9 0 0 1-1.8 1.4H9.1v-5.6Z"
		/>
	{:else if id === "love"}
		<path
			fill="#FF4B55"
			d="M12 21 3.9 12.9A5.2 5.2 0 0 1 11.2 5.5l.8.8.8-.8a5.2 5.2 0 0 1 7.3 7.4L12 21Z"
		/>
		<path fill="#fff" fill-opacity="0.18" d="M12 7.1 11.2 6a4 4 0 0 0-5.4 5.9l6.2 6.2 6.2-6.2A4 4 0 0 0 12.8 6L12 7.1Z" />
	{:else if id === "laugh"}
		<circle cx="12" cy="12" r="10" fill="#FFC83D" />
		<path d="M4.7 11.1c.8 1.3 1 2.6 0 3.6-1-1-.8-2.3 0-3.6Z" fill="#5AC8FA" />
		<path d="M19.3 11.1c.8 1.3 1 2.6 0 3.6-1-1-.8-2.3 0-3.6Z" fill="#5AC8FA" />
		<path d="M6.6 10.4c.7-1 1.9-1.6 3.1-1.4M17.4 10.4c-.7-1-1.9-1.6-3.1-1.4" stroke="#5A3300" stroke-width="1.3" stroke-linecap="round" fill="none" />
		<path d="M6.7 13.1h10.6c-.5 3.1-2.8 5-5.3 5s-4.8-1.9-5.3-5Z" fill="#5A3300" />
		<path d="M9 17.7c.9.7 1.9 1.1 3 1.1s2.1-.4 3-1.1c-.6-1-1.8-1.6-3-1.6s-2.4.6-3 1.6Z" fill="#FF7A85" />
	{:else if id === "wow"}
		<circle cx="12" cy="12" r="10" fill="#FFC83D" />
		<path d="M6.7 7.9c.8-.8 2-1.1 3.1-.8M17.3 7.9c-.8-.8-2-1.1-3.1-.8" stroke="#7A5300" stroke-width="1.2" stroke-linecap="round" fill="none" />
		<circle cx="8.9" cy="10.4" r="1.5" fill="#5A3300" />
		<circle cx="15.1" cy="10.4" r="1.5" fill="#5A3300" />
		<ellipse cx="12" cy="15.6" rx="2.3" ry="2.9" fill="#5A3300" />
	{:else if id === "celebrate"}
		<path d="M2.6 21.4 7.7 8.9l7.4 7.4-12.5 5.1Z" fill="#FFC83D" />
		<path d="M2.6 21.4 5.9 13.3l4.9 4.9-8.2 3.2Z" fill="#F0A500" />
		<circle cx="16.4" cy="4.8" r="1.2" fill="#FF4B55" />
		<circle cx="20.2" cy="8.4" r="1.2" fill="#5AC8FA" />
		<circle cx="19.3" cy="3.4" r="1" fill="#7C4DFF" />
		<circle cx="13.4" cy="3.6" r="1" fill="#34C759" />
		<rect x="20.4" y="12" width="2.1" height="2.1" rx="0.5" transform="rotate(24 21.4 13)" fill="#FFD60A" />
		<path d="M14.8 8.2 17 6M17.8 11.1 20 10" stroke="#34C759" stroke-width="1.1" stroke-linecap="round" />
	{:else if id === "fire"}
		<defs>
			<linearGradient id="reactFireGrad" x1="12" y1="21" x2="12" y2="3" gradientUnits="userSpaceOnUse">
				<stop stop-color="#FF3D00" />
				<stop offset="0.55" stop-color="#FF9100" />
				<stop offset="1" stop-color="#FFD54F" />
			</linearGradient>
		</defs>
		<path
			fill="url(#reactFireGrad)"
			d="M12.5 2c.4 2.9 2.6 4.4 3.6 6.7 1.6 3.6-.4 8.3-4.1 8.3-3 0-5.3-2.4-5.3-5.6 0-2.1 1.1-3.6 2-5.1.4 1.3 1.2 1.9 2.1 2.7.9-1.9 1-4.1-.4-7 .8.5 1.5 1.6 2.1 3 .5-1.3.3-2.7-.1-4Z"
		/>
		<path fill="#FFE082" d="M12 20a3 3 0 0 1-3-3c0-1.5 1-2.6 1.7-3.7.4 1.1 1 1.6 1.6 2.3.6-.9.7-1.9.6-2.9 1 .9 2 2.3 2 3.9a2.9 2.9 0 0 1-2.9 3.4Z" />
	{/if}
</svg>
