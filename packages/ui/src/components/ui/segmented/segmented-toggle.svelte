<script lang="ts" module>
export interface SegmentedToggleProps {
	/** Current checked state. */
	checked: boolean;
	onCheckedChange: (next: boolean) => void;
	/** Label for the "on" segment. Default `On`. */
	onLabel?: string;
	/** Label for the "off" segment. Default `Off`. */
	offLabel?: string;
	size?: "xs" | "sm" | "md";
	/** Stretch to fill parent width. Default false (compact). */
	fill?: boolean;
	disabled?: boolean;
	class?: string;
	"aria-label"?: string;
	/** Point at a visible label instead of duplicating it as `aria-label`. */
	"aria-labelledby"?: string;
}
</script>

<script lang="ts">
	import Segmented from "./segmented.svelte";

	let {
		checked,
		onCheckedChange,
		onLabel = "On",
		offLabel = "Off",
		size = "sm",
		fill = false,
		disabled = false,
		class: className,
		"aria-label": ariaLabel,
		"aria-labelledby": ariaLabelledby,
	}: SegmentedToggleProps = $props();

	// Thin shorthand around <Segmented> for the canonical Off/On pattern, so consumers skip the options array.
	const options = $derived([
		{ value: "off", label: offLabel },
		{ value: "on", label: onLabel },
	]);
</script>

<Segmented
	{options}
	value={checked ? "on" : "off"}
	onValueChange={(v) => onCheckedChange(v === "on")}
	{size}
	{fill}
	{disabled}
	class={className}
	aria-label={ariaLabel}
	aria-labelledby={ariaLabelledby}
/>
