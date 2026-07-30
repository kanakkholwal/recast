<script lang="ts">
import { Trash2 } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";

interface Props {
	count: number;
	allSelected: boolean;
	/** False when the filtered list is empty, so Select all has nothing to do. */
	canSelectAll: boolean;
	onToggleAll: () => void;
	onDelete: () => void;
	onCancel: () => void;
}

let { count, allSelected, canSelectAll, onToggleAll, onDelete, onCancel }: Props = $props();
</script>

<div
  in:fly={{ y: 24, duration: 220, easing: cubicOut }}
  out:fly={{ y: 24, duration: 160, easing: cubicOut }}
  class="fixed inset-x-0 bottom-6 z-40 flex justify-center px-6"
>
  <div
    role="toolbar"
    aria-label="Selection actions"
    class="flex items-center gap-1.5 rounded-2xl border border-border bg-card/95 p-1.5 px-5 shadow-2xl ring-1 ring-border/40 backdrop-blur-xl"
  >
    <span class="text-[12px] font-medium tabular-nums text-foreground" aria-live="polite">
      {count} selected
    </span>
    <div class="mx-1 h-4 w-px bg-border/60"></div>
    <Button
      variant="ghost"
      size="xs"
      class="h-7 text-[11px]"
      onclick={onToggleAll}
      disabled={!canSelectAll}
    >
      {allSelected ? "Clear all" : "Select all"}
    </Button>
    <Button
      variant="destructive"
      size="xs"
      class="h-7 gap-1.5 text-[11px]"
      onclick={onDelete}
      disabled={count === 0}
    >
      <Trash2 size={12} />
      Delete{count > 0 ? ` (${count})` : ""}
    </Button>
    <Button
      variant="ghost"
      size="xs"
      class="h-7 text-[11px] text-muted-foreground hover:text-foreground"
      onclick={onCancel}
    >
      Cancel
    </Button>
  </div>
</div>
