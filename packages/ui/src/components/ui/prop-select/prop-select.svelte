<script lang="ts" module>
export interface PropSelectOption {
	value: string;
	label: string;
}
export interface PropSelectProps {
	label: string;
	value: string;
	options: PropSelectOption[];
	onChange: (value: string) => void;
	disabled?: boolean;
	class?: string;
}
</script>

<script lang="ts">
	import * as Select from "@recast/ui/select";
	import { cn } from "@recast/ui/utils";

	let { label, value, options, onChange, disabled = false, class: className }: PropSelectProps =
		$props();

	const selected = $derived(options.find((o) => o.value === value));
</script>

<Select.Root type="single" {value} onValueChange={(v) => v && onChange(v)}>
	<Select.Trigger
		aria-label={label}
		{disabled}
		class={cn(
			"bg-muted/60 ring-border/40 text-foreground hover:bg-muted focus-visible:ring-ring/60 h-8 min-h-0 w-full border-transparent py-0 pr-2 pl-2.5 text-xs leading-none font-medium ring-1 ring-inset transition-colors",
			className,
		)}
	>
		<span class="truncate">{selected?.label ?? "Select"}</span>
	</Select.Trigger>
	<Select.Content class="text-xs">
		{#each options as opt (opt.value)}
			<Select.Item value={opt.value} label={opt.label}>{opt.label}</Select.Item>
		{/each}
	</Select.Content>
</Select.Root>
