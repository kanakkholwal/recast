<script lang="ts">
import * as Select from "@recast/ui/select";
import { cn } from "@recast/ui/utils";

// Inline select on the shared field surface, for a PropRow; never a native <select>.
interface Option {
	value: string;
	label: string;
}
interface Props {
	label: string;
	value: string;
	options: Option[];
	onChange: (value: string) => void;
	disabled?: boolean;
	class?: string;
}

let { label, value, options, onChange, disabled = false, class: className }: Props = $props();

const selected = $derived(options.find((o) => o.value === value));
</script>

<Select.Root type="single" {value} onValueChange={(v) => v && onChange(v)}>
	<Select.Trigger
		aria-label={label}
		{disabled}
		class={cn(
			"h-8 min-h-0 w-full border-transparent bg-muted/60 py-0 pl-2.5 pr-2 text-xs font-medium leading-none text-foreground ring-1 ring-inset ring-border/40 transition-colors hover:bg-muted focus-visible:ring-ring/60 dark:bg-muted/60 dark:hover:bg-muted",
			className,
		)}
	>
		<span class="truncate">{selected?.label ?? "Select"}</span>
	</Select.Trigger>
	<Select.Content class="text-[12px]">
		{#each options as opt (opt.value)}
			<Select.Item value={opt.value} label={opt.label}>{opt.label}</Select.Item>
		{/each}
	</Select.Content>
</Select.Root>
