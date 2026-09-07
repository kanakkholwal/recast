<script lang="ts" module>
export interface SliderRowProps {
	label: string;
	value: number;
	min?: number;
	max?: number;
	step?: number;
	unit?: string;
	/** Surfaced as the track's title tooltip; adds no row height. */
	description?: string;
	disabled?: boolean;
	onstart?: () => void;
	onchange: (value: number) => void;
	formatValue?: (value: number, unit: string) => string;
}
</script>

<script lang="ts">
	import { PropRow } from "@recast/ui/prop-row";
	import { SliderControl } from "@recast/ui/slider-control";

	let {
		label,
		value,
		min,
		max,
		step,
		unit = "",
		description,
		disabled = false,
		onstart,
		onchange,
		formatValue,
	}: SliderRowProps = $props();
</script>

<PropRow {label}>
	<SliderControl
		dense
		hideLabel
		class="flex-1"
		{label}
		{value}
		{min}
		{max}
		{step}
		{unit}
		{description}
		{disabled}
		{onstart}
		{onchange}
		{formatValue}
	/>
</PropRow>
