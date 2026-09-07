<script lang="ts">
let {
	class: klass = "",
	size = 24,
	color = "currentColor",
	"aria-hidden": ariaHidden = "true",
}: {
	class?: string;
	size?: number | string;
	color?: string;
	"aria-hidden"?: boolean | "true" | "false";
} = $props();
</script>

<!--
Reserved accent slot. Add a Phosphor duotone SVG here when a new AI
touchpoint needs visual differentiation from the Tabler outline icons.
-->
<svg
	xmlns="http://www.w3.org/2000/svg"
	viewBox="0 0 256 256"
	width={size}
	height={size}
	fill="none"
	aria-hidden={ariaHidden}
	class={klass}
></svg>
