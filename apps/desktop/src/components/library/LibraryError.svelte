<script lang="ts">
// A failed scan is not an empty library. Without this the pages fell through
// to the empty state, telling the user their disk was empty when the scan had
// simply thrown — and the toast that said otherwise was long gone.
import { RefreshCw, TriangleAlert } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { fade } from "svelte/transition";

interface Props {
	title: string;
	/** The underlying failure, shown verbatim so it can be reported. */
	message: string;
	onRetry: () => void;
}

let { title, message, onRetry }: Props = $props();
</script>

<div
  in:fade={{ duration: 200 }}
  class="flex flex-col items-center gap-3 rounded-xl border border-dashed border-destructive/40 bg-destructive/5 p-12 text-center"
  role="alert"
>
  <div class="flex size-12 items-center justify-center rounded-xl bg-destructive/10 text-destructive">
    <TriangleAlert class="size-5" />
  </div>
  <div>
    <p class="text-[14px] font-semibold text-foreground">{title}</p>
    <p class="mt-1 max-w-md text-[11.5px] text-muted-foreground">{message}</p>
  </div>
  <Button variant="secondary" size="sm" class="gap-1.5" onclick={onRetry}>
    <RefreshCw class="size-3.5" />
    Try again
  </Button>
</div>
