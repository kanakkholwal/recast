<script lang="ts">
// Brand mark for Windows, with the same Lucide-compatible prop surface as the others. Path from simple-icons.
import type { SVGAttributes } from "svelte/elements";

interface Props extends SVGAttributes<SVGSVGElement> {
	size?: number | string;
}

let { size = 24, ...rest }: Props = $props();
</script>

<svg
	xmlns="http://www.w3.org/2000/svg"
	width={size}
	height={size}
	viewBox="0 0 24 24"
	fill="currentColor"
	aria-hidden="true"
	{...rest}
>
	<path
		d="M0 3.45L9.75 2.1v9.45H0M10.95 1.95L24 0v11.4H10.95M0 12.6h9.75v9.45L0 20.7M10.95 12.6L24 12.6v11.4L10.95 21.9"
	/>
</svg>