<script lang="ts">
// One sort vocabulary for every library page. The two pages used to disagree
// on the Size icon (Film vs Download) for the same concept.
import { Clock, HardDrive, SortAsc } from "@recast/icons";
import * as Select from "@recast/ui/select";
import type { LibrarySort } from "$lib/library/list";

interface Props {
	value: LibrarySort;
	/** Noun for the control's label, e.g. "recordings". */
	noun: string;
}

let { value = $bindable("recent"), noun }: Props = $props();

const LABELS: Record<LibrarySort, string> = {
	recent: "Recent",
	name: "Name",
	size: "Size",
};
</script>

<Select.Root
  type="single"
  {value}
  onValueChange={(v: string) => {
    if (v === "recent" || v === "name" || v === "size") value = v;
  }}
>
  <Select.Trigger
    size="sm"
    class="h-9! gap-1.5 rounded-lg border-transparent bg-muted/60 px-3 text-[12px] font-medium text-foreground ring-1 ring-inset ring-border/40 hover:bg-muted"
    aria-label={`Sort ${noun}`}
  >
    <span data-slot="select-value" class="flex items-center gap-1.5">
      <SortAsc size={12} class="text-muted-foreground" />
      {LABELS[value]}
    </span>
  </Select.Trigger>
  <Select.Content align="end" sideOffset={6} class="w-36 p-1">
    <Select.Item value="recent" label="Recent" class="text-[11.5px]">
      <Clock class="size-3 text-muted-foreground" />
      Recent
    </Select.Item>
    <Select.Item value="name" label="Name" class="text-[11.5px]">
      <SortAsc class="size-3 text-muted-foreground" />
      Name
    </Select.Item>
    <Select.Item value="size" label="Size" class="text-[11.5px]">
      <HardDrive class="size-3 text-muted-foreground" />
      Size
    </Select.Item>
  </Select.Content>
</Select.Root>
