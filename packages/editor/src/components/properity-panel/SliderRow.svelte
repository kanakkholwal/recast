<script lang="ts">
import { SliderControl } from "@recast/ui/slider-control";
import PropRow from "./PropRow.svelte";

// Dense inline slider: external label + compact track, the one shape every panel uses.
interface Props {
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
}: Props = $props();
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
