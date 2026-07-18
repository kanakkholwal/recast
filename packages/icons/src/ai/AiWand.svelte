<script lang="ts">
	let {
		class: klass = "",
		size = 24,
		color = "currentColor",
		"aria-hidden": ariaHidden = "true",
	}: { class?: string; size?: number | string; color?: string; "aria-hidden"?: boolean | "true" | "false" } =
		$props();
</script>

<!--
AI accent: magic wand (Phosphor duotone).

Inlined SVG from `@phosphor-icons/core/duotone/magic-wand-duotone.svg`.
Vendored into source (rather than deep-importing the SVG asset) so the
bundle ships a single, static asset — no Vite `?raw` or `?url` query
needed, no runtime SVG parsing. If Phosphor ships a new weight, just
swap the SVG body and update the viewBox above.

Locked accent (see AGENTS.md §4): used in AI-action buttons — `Regenerate`,
`Re-run analysis`, `Suggest`, etc. Do not introduce new uses without
ensuring the entire touchpoint reads as a "smart / regenerate / re-analyse"
cue — the visual weight (filled back, stroke front) is intentional.
-->
<svg
	xmlns="http://www.w3.org/2000/svg"
	viewBox="0 0 256 256"
	width={size}
	height={size}
	fill={color}
	aria-hidden={ariaHidden}
	class={klass}
>
	<!-- Back layer (20% opacity, duotone convention). -->
	<path
		d="M176,112,74.34,213.66a8,8,0,0,1-11.31,0L42.34,193a8,8,0,0,1,0-11.31L144,80Z"
		opacity="0.2"
	/>
	<!-- Front layer (sharp stroke-equivalent). -->
	<path
		d="M248,152a8,8,0,0,1-8,8H224v16a8,8,0,0,1-16,0V160H192a8,8,0,0,1,0-16h16V128a8,8,0,0,1,16,0v16h16A8,8,0,0,1,248,152ZM56,72H72V88a8,8,0,0,0,16,0V72h16a8,8,0,0,0,0-16H88V40a8,8,0,0,0-16,0V56H56a8,8,0,0,0,0,16ZM184,192h-8v-8a8,8,0,0,0-16,0v8h-8a8,8,0,0,0,0,16h8v8a8,8,0,0,0,16,0v-8h8a8,8,0,0,0,0-16ZM219.31,80,80,219.31a16,16,0,0,1-22.62,0L36.68,198.63a16,16,0,0,1,0-22.63L176,36.69a16,16,0,0,1,22.63,0l20.68,20.68A16,16,0,0,1,219.31,80Zm-54.63,32L144,91.31l-96,96L68.68,208ZM208,68.69,187.31,48l-32,32L176,100.69Z"
	/>
</svg>
