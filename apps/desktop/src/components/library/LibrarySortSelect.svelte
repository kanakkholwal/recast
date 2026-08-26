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
    class="h-8 gap-1 rounded-lg border-border/50 px-2.5 text-[11.5px] font-medium text-muted-foreground hover:text-foreground"
    aria-label={`Sort ${noun}`}
  >
    <span data-slot="select-value" class="flex items-center gap-1">
      <SortAsc size={11} />
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
